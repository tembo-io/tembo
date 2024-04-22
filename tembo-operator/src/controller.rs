use chrono::{DateTime, Utc};
use futures::stream::StreamExt;

use crate::{
    apis::coredb_types::{CoreDB, CoreDBStatus, VolumeSnapshot},
    app_service::manager::reconcile_app_services,
    cloudnativepg::{
        backups::Backup,
        cnpg::{
            cnpg_cluster_from_cdb, reconcile_cnpg, reconcile_cnpg_scheduled_backup,
            reconcile_pooler,
        },
        placement::cnpg_placement::PlacementConfig,
        VOLUME_SNAPSHOT_CLASS_NAME,
    },
    config::Config,
    exec::{ExecCommand, ExecOutput},
    extensions::database_queries::is_not_restarting,
    heartbeat::reconcile_heartbeat,
    ingress::reconcile_postgres_ing_route_tcp,
    postgres_certificates::reconcile_certificates,
    psql::{PsqlCommand, PsqlOutput},
    secret::{reconcile_postgres_role_secret, reconcile_secret},
    telemetry, Error, Metrics, Result,
};
use k8s_openapi::{
    api::core::v1::{Namespace, Pod},
    apimachinery::pkg::util::intstr::IntOrString,
};
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
        wait::Condition,
        watcher::Config as watcherConfig,
    },
    Resource,
};

use crate::cloudnativepg::hibernate::reconcile_cluster_hibernation;
use crate::{
    apis::postgres_parameters::PgConfig,
    configmap::reconcile_generic_metrics_configmap,
    extensions::{database_queries::list_config_params, reconcile_extensions},
    ingress::{reconcile_extra_postgres_ing_route_tcp, reconcile_ip_allowlist_middleware},
    network_policies::reconcile_network_policies,
    postgres_exporter::reconcile_metrics_configmap,
    trunk::{extensions_that_require_load, reconcile_trunk_configmap},
};
use rand::Rng;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::*;

pub static COREDB_FINALIZER: &str = "coredbs.coredb.io";
pub static COREDB_ANNOTATION: &str = "coredbs.coredb.io/watch";

// Context for our reconciler
#[derive(Clone)]
pub struct Context {
    /// Kubernetes client
    pub client: Client,
    /// Diagnostics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prometheus metrics
    pub metrics: Metrics,
}

pub fn requeue_normal_with_jitter() -> Action {
    let cfg = Config::default();
    // Check back every 90-150 seconds
    let jitter = rand::thread_rng().gen_range(0..60);
    Action::requeue(Duration::from_secs(cfg.reconcile_ttl + jitter))
}

#[instrument(skip(ctx, cdb), fields(trace_id))]
async fn reconcile(cdb: Arc<CoreDB>, ctx: Arc<Context>) -> Result<Action> {
    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let cfg = Config::default();
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();
    let ns = cdb.namespace().unwrap(); // cdb is namespace scoped
    let coredbs: Api<CoreDB> = Api::namespaced(ctx.client.clone(), &ns);
    // Get metadata for the CoreDB object
    let metadata = cdb.meta();
    // Get annotations from the metadata
    let annotations = metadata.annotations.clone().unwrap_or_default();

    // Check the annotations to see if it exists and check it's value
    if let Some(value) = annotations.get(COREDB_ANNOTATION) {
        // If the value is false, then we should skip reconciling
        if value == "false" {
            info!(
                "Skipping reconciliation for CoreDB \"{}\" in {}",
                cdb.name_any(),
                ns
            );
            return Ok(Action::await_change());
        }
    }

    debug!("Reconciling CoreDB \"{}\" in {}", cdb.name_any(), ns);
    finalizer(&coredbs, COREDB_FINALIZER, cdb, |event| async {
        match event {
            Finalizer::Apply(cdb) => match cdb.reconcile(ctx.clone(), &cfg).await {
                Ok(action) => Ok(action),
                Err(requeue_action) => Ok(requeue_action),
            },
            Finalizer::Cleanup(cdb) => cdb.cleanup(ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::FinalizerError(Box::new(e)))
}

pub(crate) fn error_policy(cdb: Arc<CoreDB>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(&cdb, error);

    // Check for 429 error code from Kubernetes API
    match error {
        Error::KubeError(kube_error) => match kube_error {
            kube::Error::Api(api_error) if api_error.code == 429 => {
                // Error is a 429 (too many requests), calculate backoff and jitter
                let backoff: u64 = 60;
                let max_jitter: u64 = 120;
                let jitter: u64 = rand::thread_rng().gen_range(0..=max_jitter);
                let backoff_with_jitter = Duration::from_secs(backoff + jitter);
                // Log the 429 error and the calculated backoff time
                warn!(
                    "Received HTTP 429 Too Many Requests. Requeuing after {} seconds.",
                    backoff_with_jitter.as_secs()
                );
                Action::requeue(backoff_with_jitter)
            }
            _ => Action::requeue(Duration::from_secs(5 * 60)),
        },
        _ => Action::requeue(Duration::from_secs(5 * 60)),
    }
}

// create_volume_snapshot_patch creates a patch for the CoreDB spec to enable or disable volumesnapshots
// based off the value of cfg.enable_volume_snapshot.
fn create_volume_snapshot_patch(cfg: &Config) -> serde_json::Value {
    json!({
        "spec": {
            "backup": {
                "volumeSnapshot": {
                    "enabled": cfg.enable_volume_snapshot,
                    "snapshotClass": if cfg.enable_volume_snapshot {
                        Some(VOLUME_SNAPSHOT_CLASS_NAME.to_string())
                    } else {
                        None
                    }
                }
            }
        }
    })
}

// is_volume_snapshot_update_needed checks if the volume snapshot needs to be updated in the CoreDB spec.
fn is_volume_snapshot_update_needed(
    volume_snapshot: Option<&VolumeSnapshot>,
    enable_volume_snapshot: bool,
) -> bool {
    let current_enabled = volume_snapshot.map(|vs| vs.enabled).unwrap_or(false);
    current_enabled != enable_volume_snapshot
}

impl CoreDB {
    // Reconcile (for non-finalizer related changes)
    #[instrument(skip(self, ctx, cfg))]
    async fn reconcile(&self, ctx: Arc<Context>, cfg: &Config) -> Result<Action, Action> {
        let client = ctx.client.clone();
        let _recorder = ctx.diagnostics.read().await.recorder(client.clone(), self);
        let ns = self.namespace().unwrap();
        let name = self.name_any();
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &ns);

        // If the cluster is stopped, apply hibernation and exit
        reconcile_cluster_hibernation(self, &ctx).await?;

        // Setup Node/Pod Placement Configuration for the Pooler and App Service deployments
        let placement_config = PlacementConfig::new(self);

        reconcile_network_policies(ctx.client.clone(), &ns).await?;

        // Fetch any metadata we need from Trunk
        reconcile_trunk_configmap(ctx.client.clone(), &ns).await?;

        reconcile_certificates(ctx.client.clone(), self, &ns).await?;

        // Ingress
        match std::env::var("DATA_PLANE_BASEDOMAIN") {
            Ok(basedomain) => {
                debug!(
                    "DATA_PLANE_BASEDOMAIN is set to {}, reconciling IngressRouteTCP and MiddlewareTCP for {}",
                    basedomain, name.clone()
                );

                let middleware_name = reconcile_ip_allowlist_middleware(self, ctx.clone())
                    .await
                    .map_err(|e| {
                        error!("Error reconciling MiddlewareTCP for {}: {:?}", name, e);
                        Action::requeue(Duration::from_secs(300))
                    })?;

                let service_name_read_only = format!("{}-ro", self.name_any().as_str());
                let prefix_read_only = format!("{}-ro-", self.name_any().as_str());
                let read_only_subdomain = format!("{}-ro", self.name_any().as_str());
                reconcile_postgres_ing_route_tcp(
                    self,
                    ctx.clone(),
                    &read_only_subdomain,
                    basedomain.as_str(),
                    ns.as_str(),
                    prefix_read_only.as_str(),
                    service_name_read_only.as_str(),
                    IntOrString::Int(5432),
                    vec![middleware_name.clone()],
                )
                .await
                .map_err(|e| {
                    error!("Error reconciling postgres ingress route: {:?}", e);
                    // For unexpected errors, we should requeue for several minutes at least,
                    // for expected, "waiting" type of requeuing, those should be shorter, just a few seconds.
                    // IngressRouteTCP does not have expected errors during reconciliation.
                    Action::requeue(Duration::from_secs(300))
                })?;

                let service_name_read_write = format!("{}-rw", self.name_any().as_str());
                let prefix_read_write = format!("{}-rw-", self.name_any().as_str());
                reconcile_postgres_ing_route_tcp(
                    self,
                    ctx.clone(),
                    self.name_any().as_str(),
                    basedomain.as_str(),
                    ns.as_str(),
                    prefix_read_write.as_str(),
                    service_name_read_write.as_str(),
                    IntOrString::Int(5432),
                    vec![middleware_name.clone()],
                )
                .await
                .map_err(|e| {
                    error!("Error reconciling postgres ingress route: {:?}", e);
                    // For unexpected errors, we should requeue for several minutes at least,
                    // for expected, "waiting" type of requeuing, those should be shorter, just a few seconds.
                    // IngressRouteTCP does not have expected errors during reconciliation.
                    Action::requeue(Duration::from_secs(300))
                })?;

                reconcile_extra_postgres_ing_route_tcp(
                    self,
                    ctx.clone(),
                    ns.as_str(),
                    service_name_read_write.as_str(),
                    IntOrString::Int(5432),
                    vec![middleware_name.clone()],
                )
                .await
                .map_err(|e| {
                    error!("Error reconciling extra postgres ingress route: {:?}", e);
                    // For unexpected errors, we should requeue for several minutes at least,
                    // for expected, "waiting" type of requeuing, those should be shorter, just a few seconds.
                    // IngressRouteTCP does not have expected errors during reconciliation.
                    Action::requeue(Duration::from_secs(300))
                })?;
                // If pooler is enabled, reconcile ingress route tcp for pooler
                if self.spec.connectionPooler.enabled {
                    let name_pooler = format!("{}-pooler", self.name_any().as_str());
                    let prefix_pooler = format!("{}-pooler-", self.name_any().as_str());
                    reconcile_postgres_ing_route_tcp(
                        self,
                        ctx.clone(),
                        name_pooler.as_str(),
                        basedomain.as_str(),
                        ns.as_str(),
                        prefix_pooler.as_str(),
                        name_pooler.as_str(),
                        IntOrString::Int(5432),
                        vec![middleware_name.clone()],
                    )
                    .await
                    .map_err(|e| {
                        error!("Error reconciling pooler ingress route: {:?}", e);
                        // For unexpected errors, we should requeue for several minutes at least,
                        // for expected, "waiting" type of requeuing, those should be shorter, just a few seconds.
                        // IngressRouteTCP does not have expected errors during reconciliation.
                        Action::requeue(Duration::from_secs(300))
                    })?;
                }
            }
            Err(_e) => {
                warn!(
                    "DATA_PLANE_BASEDOMAIN is not set, skipping reconciliation of IngressRouteTCP"
                );
            }
        };

        debug!("Reconciling secret");
        // Superuser connection info
        reconcile_secret(self, ctx.clone()).await?;
        reconcile_app_services(self, ctx.clone(), placement_config.clone()).await?;

        if self
            .spec
            .metrics
            .as_ref()
            .and_then(|m| m.queries.as_ref())
            .is_some()
        {
            debug!("Reconciling prometheus configmap");
            reconcile_metrics_configmap(self, client.clone(), &ns)
                .await
                .map_err(|e| {
                    error!("Error reconciling prometheus configmap: {:?}", e);
                    Action::requeue(Duration::from_secs(300))
                })?;
        }

        let _ = reconcile_postgres_role_secret(
            self,
            ctx.clone(),
            "readonly",
            &format!("{}-ro", name.clone()),
        )
        .await
        .map_err(|e| {
            error!("Error reconciling postgres exporter secret: {:?}", e);
            Action::requeue(Duration::from_secs(300))
        })?;

        reconcile_generic_metrics_configmap(self, ctx.clone()).await?;

        // Before we reconcile CNPG, we need to make sure that spec.backup.volumeSnapshot is
        // enabled in the CoreDB spec if cfg.enable_volume_snapshot = true.  If it's not
        // then we should enable it, otherwise it should be a no-op.
        self.enable_volume_snapshot(cfg, ctx.clone()).await?;

        reconcile_cnpg(self, ctx.clone()).await?;
        if cfg.enable_backup {
            reconcile_cnpg_scheduled_backup(self, ctx.clone()).await?;
        }

        // Cleanup old Postgres Exporter Deployments, Service, ServiceAccount, Role and RoleBinding
        crate::deployment_postgres_exporter::cleanup_postgres_exporter(self, ctx.clone())
            .await
            .map_err(|e| {
                error!("Error reconciling prometheus exporter deployment: {:?}", e);
                Action::requeue(Duration::from_secs(300))
            })?;

        // Reconcile Pooler resource
        reconcile_pooler(self, ctx.clone(), placement_config.clone()).await?;

        // Check if Postgres is already running
        let pg_postmaster_start_time = is_not_restarting(self, ctx.clone(), "postgres").await?;

        let patch_status = json!({
            "apiVersion": "coredb.io/v1alpha1",
            "kind": "CoreDB",
            "status": {
                "running": true,
                "pg_postmaster_start_time": pg_postmaster_start_time,
            }
        });
        patch_cdb_status_merge(&coredbs, &name, patch_status).await?;
        let (trunk_installs, extensions) =
            reconcile_extensions(self, ctx.clone(), &coredbs, &name).await?;

        let recovery_time = self.get_recovery_time(ctx.clone()).await?;

        let current_config_values = get_current_config_values(self, ctx.clone()).await?;
        let mut new_status = CoreDBStatus {
            running: true,
            extensionsUpdating: false,
            storage: Some(self.spec.storage.clone()),
            extensions: Some(extensions),
            trunk_installs: Some(trunk_installs),
            resources: Some(self.spec.resources.clone()),
            runtime_config: Some(current_config_values),
            first_recoverability_time: recovery_time,
            pg_postmaster_start_time,
            last_fully_reconciled_at: None,
        };

        let current_time = Utc::now();
        new_status.last_fully_reconciled_at = {
            let current_fully_reconciled_at = match self.status.as_ref() {
                None => None,
                Some(status) => status.last_fully_reconciled_at,
            };
            // Update the timestamp if it's been more than 30 seconds since the last update
            if current_fully_reconciled_at.map_or(true, |last_reconciled| {
                current_time > last_reconciled + Duration::from_secs(cfg.reconcile_timestamp_ttl)
            }) {
                Some(current_time)
            } else {
                current_fully_reconciled_at
            }
        };

        debug!("Updating CoreDB status to {:?} for {name}", new_status);

        let patch_status = json!({
            "apiVersion": "coredb.io/v1alpha1",
            "kind": "CoreDB",
            "status": new_status
        });

        patch_cdb_status_merge(&coredbs, &name, patch_status).await?;

        reconcile_heartbeat(self, ctx.clone()).await?;

        info!("Fully reconciled {}", self.name_any());
        Ok(requeue_normal_with_jitter())
    }

    // enable_volume_snapshot makes sure that the CoreDB spec has the spec.backup.volumeSnapshot
    // enabled.  If it's already enabled, then do nothing.
    #[instrument(skip(self, ctx))]
    async fn enable_volume_snapshot(&self, cfg: &Config, ctx: Arc<Context>) -> Result<(), Action> {
        let client = ctx.client.clone();
        let name = self.name_any();
        let namespace = self.metadata.namespace.as_ref().ok_or_else(|| {
            error!("CoreDB namespace is empty for instance: {}.", name);
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

        // Setup the client for the CoreDB
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), namespace);

        // Check if an update is needed based on the current value and the desired value from the config
        if !is_volume_snapshot_update_needed(
            self.spec.backup.volume_snapshot.as_ref(),
            cfg.enable_volume_snapshot,
        ) {
            return Ok(());
        }

        // Create the patch to update the spec.backup.volumeSnapshot based on the config
        let patch = create_volume_snapshot_patch(cfg);
        let patch_params = PatchParams {
            field_manager: Some("cntrlr".to_string()),
            ..PatchParams::default()
        };
        let patch_status = Patch::Merge(patch.clone());
        match coredbs.patch(&name, &patch_params, &patch_status).await {
            Ok(_) => {
                debug!("Successfully updated CoreDB status for {}", name);
                Ok(())
            }
            Err(e) => {
                error!("Error updating CoreDB status for {}: {:?}", name, e);
                Err(Action::requeue(Duration::from_secs(10)))
            }
        }
    }

    // Finalizer cleanup (the object was deleted, ensure nothing is orphaned)
    #[instrument(skip(self, ctx))]
    async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action> {
        // If namespace is terminating, do not publish delete event. Attempting to publish an event
        // in a terminating namespace will leave us in a bad state in which the namespace will hang
        // in terminating state.
        let ns_api: Api<Namespace> = Api::all(ctx.client.clone());
        let ns_status = ns_api
            .get_status(self.metadata.namespace.as_ref().unwrap())
            .await
            .map_err(Error::KubeError);
        let phase = ns_status.unwrap().status.unwrap().phase;
        if phase == Some("Terminating".to_string()) {
            return Ok(Action::await_change());
        }
        let recorder = ctx
            .diagnostics
            .read()
            .await
            .recorder(ctx.client.clone(), self);
        // CoreDB doesn't have dependencies in this example case, so we just publish an event
        recorder
            .publish(Event {
                type_: EventType::Normal,
                reason: "DeleteCoreDB".into(),
                note: Some(format!("Delete `{}`", self.name_any())),
                action: "Reconciling".into(),
                secondary: None,
            })
            .await
            .map_err(Error::KubeError)?;
        Ok(Action::await_change())
    }

    #[instrument(skip(self, client))]
    async fn primary_pod_cnpg_conditional_readiness(
        &self,
        client: Client,
        wait_for_ready: bool,
    ) -> Result<Pod, Action> {
        let requires_load =
            extensions_that_require_load(client.clone(), &self.metadata.namespace.clone().unwrap())
                .await?;
        let cluster = cnpg_cluster_from_cdb(self, None, requires_load);
        let cluster_name = cluster.metadata.name.as_ref().ok_or_else(|| {
            error!(
                "CNPG Cluster name is empty for instance: {}.",
                self.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;
        let namespace = self.metadata.namespace.as_ref().ok_or_else(|| {
            error!(
                "CoreDB namespace is empty for instance: {}.",
                self.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;
        let cluster_selector = format!("cnpg.io/cluster={}", cluster_name);
        let role_selector = "role=primary";
        let list_params = ListParams::default()
            .labels(&cluster_selector)
            .labels(role_selector);
        let pods: Api<Pod> = Api::namespaced(client, namespace);
        let pods = pods.list(&list_params);
        // Return an error if the query fails
        let pod_list = pods.await.map_err(|_e| {
            // It is not expected to fail the query to the pods API
            error!(
                "Failed to query for CNPG primary pod of {}",
                &self.name_any()
            );
            Action::requeue(Duration::from_secs(300))
        })?;
        // Return an error if the list is empty
        if pod_list.items.is_empty() {
            // It's expected to sometimes be empty, we should retry after a short duration
            warn!("Failed to find CNPG primary pod of {}, this can be expected if the pod is restarting for some reason", &self.name_any());
            return Err(Action::requeue(Duration::from_secs(5)));
        }
        let primary = pod_list.items[0].clone();

        if wait_for_ready && !is_postgres_ready().matches_object(Some(&primary)) {
            // It's expected to sometimes be empty, we should retry after a short duration
            warn!(
                "Found CNPG primary pod of {}, but it is not ready",
                &self.name_any()
            );
            return Err(Action::requeue(Duration::from_secs(5)));
        }

        Ok(primary)
    }

    #[instrument(skip(self, client))]
    pub async fn primary_pod_cnpg(&self, client: Client) -> Result<Pod, Action> {
        self.primary_pod_cnpg_conditional_readiness(client, true)
            .await
    }

    #[instrument(skip(self, client))]
    pub async fn primary_pod_cnpg_ready_or_not(&self, client: Client) -> Result<Pod, Action> {
        self.primary_pod_cnpg_conditional_readiness(client, false)
            .await
    }

    #[instrument(skip(self, client))]
    async fn pods_by_cluster_conditional_readiness(
        &self,
        client: Client,
        wait_for_ready: bool,
    ) -> Result<Vec<Pod>, Action> {
        let requires_load =
            extensions_that_require_load(client.clone(), &self.metadata.namespace.clone().unwrap())
                .await?;
        let cluster = cnpg_cluster_from_cdb(self, None, requires_load);
        let cluster_name = cluster.metadata.name.as_ref().ok_or_else(|| {
            error!(
                "CNPG Cluster name is empty for instance: {}.",
                self.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;
        let namespace = self.metadata.namespace.as_ref().ok_or_else(|| {
            error!(
                "CoreDB namespace is empty for instance: {}.",
                self.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

        // Added role labels here
        let cluster_selector =
            format!("cnpg.io/cluster={cluster_name},cnpg.io/podRole=instance,role=primary");
        let replica_selector = format!("cnpg.io/cluster={cluster_name},role=replica");

        let list_params_cluster = ListParams::default().labels(&cluster_selector);
        let list_params_replica = ListParams::default().labels(&replica_selector);

        let pods: Api<Pod> = Api::namespaced(client, namespace);
        let primary_pods = pods.list(&list_params_cluster);
        let replica_pods = pods.list(&list_params_replica);

        let primary_pod_list = primary_pods.await.map_err(|_e| {
            error!(
                "Failed to query for CNPG primary pods of {}",
                &self.name_any()
            );
            Action::requeue(Duration::from_secs(300))
        })?;

        let replica_pod_list = replica_pods.await.map_err(|_e| {
            error!(
                "Failed to query for CNPG replica pods of {}",
                &self.name_any()
            );
            Action::requeue(Duration::from_secs(300))
        })?;

        let pod_list = [primary_pod_list.items, replica_pod_list.items].concat();

        if pod_list.is_empty() {
            warn!("Failed to find CNPG pods of {}", &self.name_any());
            return Err(Action::requeue(Duration::from_secs(30)));
        }

        // Filter only pods that are ready
        let ready_pods: Vec<Pod> = pod_list
            .into_iter()
            .filter(|pod| {
                if let Some(conditions) = &pod.status.as_ref().and_then(|s| s.conditions.as_ref()) {
                    conditions
                        .iter()
                        .any(|c| c.type_ == "Ready" && c.status == "True")
                } else {
                    false
                }
            })
            .collect();

        // If the instance has a pod that is not ready and is not a restore instance, requeue
        if wait_for_ready && ready_pods.is_empty() {
            warn!("Failed to find ready CNPG pods of {}", &self.name_any());
            return Err(Action::requeue(Duration::from_secs(30)));
        }

        Ok(ready_pods)
    }

    #[instrument(skip(self, client))]
    pub async fn pods_by_cluster(&self, client: Client) -> Result<Vec<Pod>, Action> {
        self.pods_by_cluster_conditional_readiness(client, true)
            .await
    }

    #[instrument(skip(self, client))]
    pub async fn pods_by_cluster_ready_or_not(&self, client: Client) -> Result<Vec<Pod>, Action> {
        self.pods_by_cluster_conditional_readiness(client, false)
            .await
    }

    #[instrument(skip(self, client))]
    async fn check_replica_count_matches_pods(&self, client: Client) -> Result<(), Action> {
        // Fetch current replica count from Self
        let desired_replica_count = self.spec.replicas;
        debug!(
            "Instance {} has a desired replica count: {}",
            self.name_any(),
            desired_replica_count
        );

        // Fetch current pods with pods_by_cluster
        let current_pods = self.pods_by_cluster(client.clone()).await?;
        let pod_names: Vec<String> = current_pods.iter().map(|pod| pod.name_any()).collect();
        debug!(
            "Found {} pods, {:?} for {}",
            current_pods.len(),
            pod_names,
            self.name_any()
        );

        // Check if the number of running pods matches the desired replica count
        if current_pods.len() != desired_replica_count as usize {
            warn!(
                "Number of running pods ({}) does not match desired replica count ({}) for ({}). Requeuing.",
                current_pods.len(),
                desired_replica_count,
                self.name_any()
            );
            return Err(Action::requeue(Duration::from_secs(10)));
        }

        info!(
            "Number of running pods ({}) matches desired replica count ({}) for ({}).",
            current_pods.len(),
            desired_replica_count,
            self.name_any()
        );
        Ok(())
    }

    pub async fn log_pod_status(&self, client: Client, pod_name: &str) -> Result<(), kube::Error> {
        let namespace = self
            .metadata
            .namespace
            .clone()
            .expect("CoreDB should have a namespace");
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        match pods.get(pod_name).await {
            Ok(pod) => {
                let status = pod
                    .status
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "Unknown".to_string());
                debug!(
                    "Status of instance {} pod {} in namespace {}: {}",
                    self.metadata
                        .name
                        .clone()
                        .expect("CoreDB should have a name"),
                    pod_name,
                    namespace,
                    status
                );
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[instrument(skip(self, context))]
    pub async fn psql(
        &self,
        command: String,
        database: String,
        context: Arc<Context>,
    ) -> Result<PsqlOutput, Action> {
        let pod = self.primary_pod_cnpg(context.client.clone()).await?;
        let pod_name_cnpg = pod.metadata.name.as_ref().ok_or_else(|| {
            error!("Pod name is empty for instance: {}.", self.name_any());
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

        let cnpg_psql_command = PsqlCommand::new(
            pod_name_cnpg.clone(),
            self.metadata.namespace.clone().unwrap(),
            command,
            database,
            context.clone(),
        );
        debug!("Running exec command in {}", pod_name_cnpg);
        cnpg_psql_command.execute().await
    }

    pub async fn exec(
        &self,
        pod_name: String,
        client: Client,
        command: &[String],
    ) -> Result<ExecOutput, Error> {
        ExecCommand::new(pod_name, self.metadata.namespace.clone().unwrap(), client)
            .execute(command)
            .await
    }

    fn process_backups(&self, backup_list: Vec<Backup>) -> Option<DateTime<Utc>> {
        let backup = backup_list
            .iter()
            .filter_map(|backup| backup.status.as_ref())
            .filter(|status| status.phase.as_deref() == Some("completed"))
            .filter_map(|status| status.stopped_at.as_ref())
            .filter_map(|stopped_at_str| DateTime::parse_from_rfc3339(stopped_at_str).ok())
            .map(|dt_with_offset| dt_with_offset.with_timezone(&Utc))
            .min();

        backup
    }

    // get_recovery_time returns the time at which the first recovery will be possible from the
    // oldest completed Backup object in the namespace.
    #[instrument(skip(self, context))]
    pub async fn get_recovery_time(
        &self,
        context: Arc<Context>,
    ) -> Result<Option<DateTime<Utc>>, Action> {
        let client = context.client.clone();
        let namespace = self.metadata.namespace.as_ref().ok_or_else(|| {
            error!(
                "CoreDB namespace is empty for instance: {}.",
                self.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;
        let cluster_name = self.name_any();
        let backup: Api<Backup> = Api::namespaced(client, namespace);
        let lp = ListParams::default().labels(&format!("cnpg.io/cluster={}", cluster_name));
        let backup_list = backup.list(&lp).await.map_err(|e| {
            error!("Error getting backups: {:?}", e);
            Action::requeue(Duration::from_secs(300))
        })?;

        let oldest_backup_time = self.process_backups(backup_list.items);

        Ok(oldest_backup_time)
    }
}

pub fn is_pod_ready() -> impl Condition<Pod> + 'static {
    move |obj: Option<&Pod>| {
        if let Some(pod) = &obj {
            if let Some(status) = &pod.status {
                if let Some(conds) = &status.conditions {
                    if let Some(pcond) = conds.iter().find(|c| c.type_ == "ContainersReady") {
                        return pcond.status == "True";
                    }
                }
            }
        }
        false
    }
}

pub fn is_postgres_ready() -> impl Condition<Pod> + 'static {
    move |obj: Option<&Pod>| {
        if let Some(pod) = &obj {
            if let Some(status) = &pod.status {
                if let Some(container_statuses) = &status.container_statuses {
                    for container in container_statuses {
                        if container.name == "postgres" {
                            return container.ready;
                        }
                    }
                }
            }
        }
        false
    }
}

#[instrument(skip(ctx, cdb))]
pub async fn get_current_coredb_resource(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<CoreDB, Action> {
    let coredb_name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("Namespace is empty for instance: {}.", &coredb_name);
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let coredb_api: Api<CoreDB> = Api::namespaced(ctx.client.clone(), namespace);
    let coredb = coredb_api.get(&coredb_name).await.map_err(|e| {
        error!("Error getting CoreDB resource: {:?}", e);
        Action::requeue(Duration::from_secs(10))
    })?;
    Ok(coredb)
}

// Get current config values
pub async fn get_current_config_values(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<PgConfig>, Action> {
    let cfg = list_config_params(cdb, ctx.clone()).await?;
    Ok(cfg)
}

pub async fn patch_cdb_status_merge(
    cdb: &Api<CoreDB>,
    name: &str,
    patch: serde_json::Value,
) -> Result<(), Action> {
    let pp = PatchParams {
        field_manager: Some("cntrlr".to_string()),
        ..PatchParams::default()
    };
    let patch_status = Patch::Merge(patch.clone());
    match cdb.patch_status(name, &pp, &patch_status).await {
        Ok(_) => {
            debug!("Successfully updated CoreDB status for {}", name);
            Ok(())
        }
        Err(e) => {
            error!("Error updating CoreDB status for {}: {:?}", name, e);
            Err(Action::requeue(Duration::from_secs(10)))
        }
    }
}

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}
impl Default for Diagnostics {
    #[instrument]
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "coredb-controller".into(),
        }
    }
}
impl Diagnostics {
    #[instrument(skip(self, client))]
    fn recorder(&self, client: Client, cdb: &CoreDB) -> Recorder {
        Recorder::new(client, self.reporter.clone(), cdb.object_ref(&()))
    }
}

/// State shared between the controller and the web server
#[derive(Clone, Default)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics registry
    registry: prometheus::Registry,
}

/// State wrapper around the controller outputs for the web server
impl State {
    /// Metrics getter
    pub fn metrics(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    pub fn create_context(&self, client: Client) -> Arc<Context> {
        Arc::new(Context {
            client,
            metrics: Metrics::default().register(&self.registry).unwrap(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

/// Initialize the controller and shared state (given the crd is installed)
pub async fn run(state: State) {
    // Initialize the Kubernetes client
    let client_future = kube::Client::try_default();
    let client = match client_future.await {
        Ok(wrapped_client) => wrapped_client,
        Err(_) => panic!("Please configure your Kubernetes Context"),
    };

    let coredb = Api::<CoreDB>::all(client.clone());
    if let Err(e) = coredb.list(&ListParams::default().limit(1)).await {
        error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }
    Controller::new(coredb, watcherConfig::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.create_context(client))
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

// Tests rely on fixtures.rs
#[cfg(test)]
mod test {
    use super::{reconcile, Backup, Context, CoreDB};
    use crate::apis::coredb_types::VolumeSnapshot;
    use crate::cloudnativepg::{
        backups::{BackupCluster, BackupSpec, BackupStatus},
        VOLUME_SNAPSHOT_CLASS_NAME,
    };
    use crate::config::Config;
    use crate::controller::{create_volume_snapshot_patch, is_volume_snapshot_update_needed};
    use chrono::{DateTime, NaiveDate, Utc};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::sync::Arc;

    #[tokio::test]
    async fn new_coredbs_without_finalizers_gets_a_finalizer() {
        let (testctx, fakeserver, _) = Context::test();
        let coredb = CoreDB::test();
        // verify that coredb gets a finalizer attached during reconcile
        fakeserver.handle_finalizer_creation(&coredb);
        let res = reconcile(Arc::new(coredb), testctx).await;
        assert!(res.is_ok(), "initial creation succeeds in adding finalizer");
    }

    #[tokio::test]
    async fn test_process_backups() {
        let coredb = CoreDB::test();
        let backup_name = "test-backup-1".to_string();
        let namespace = "test".to_string();

        let backup_list = vec![Backup {
            metadata: ObjectMeta {
                name: Some(backup_name.clone()),
                namespace: Some(namespace),
                ..Default::default()
            },
            spec: BackupSpec {
                cluster: BackupCluster {
                    name: backup_name.clone(),
                },
                ..Default::default()
            },
            status: Some(BackupStatus {
                phase: Some("completed".to_string()),
                stopped_at: Some("2023-09-19T23:14:00Z".to_string()),
                ..Default::default()
            }),
        }];

        let oldest_backup_time = coredb.process_backups(backup_list);

        let expected_time = NaiveDate::from_ymd_opt(2023, 9, 19)
            .and_then(|date| date.and_hms_opt(23, 14, 0))
            .map(|naive_dt| DateTime::from_naive_utc_and_offset(naive_dt, Utc));
        assert_eq!(oldest_backup_time, expected_time);
    }

    #[tokio::test]
    async fn test_process_backups_multiple_backups() {
        let coredb = CoreDB::test();

        let backup_list = vec![
            Backup {
                metadata: ObjectMeta {
                    name: Some("backup-1".to_string()),
                    namespace: Some("test".to_string()),
                    ..Default::default()
                },
                spec: BackupSpec {
                    cluster: BackupCluster {
                        name: "backup-1".to_string(),
                    },
                    ..Default::default()
                },
                status: Some(BackupStatus {
                    phase: Some("completed".to_string()),
                    stopped_at: Some("2023-09-19T23:14:00Z".to_string()),
                    ..Default::default()
                }),
            },
            Backup {
                metadata: ObjectMeta {
                    name: Some("backup-2".to_string()),
                    namespace: Some("test".to_string()),
                    ..Default::default()
                },
                spec: BackupSpec {
                    cluster: BackupCluster {
                        name: "backup-2".to_string(),
                    },
                    ..Default::default()
                },
                status: Some(BackupStatus {
                    phase: Some("completed".to_string()),
                    stopped_at: Some("2023-09-18T22:12:00Z".to_string()), // This is the oldest
                    ..Default::default()
                }),
            },
            Backup {
                metadata: ObjectMeta {
                    name: Some("backup-3".to_string()),
                    namespace: Some("test".to_string()),
                    ..Default::default()
                },
                spec: BackupSpec {
                    cluster: BackupCluster {
                        name: "backup-3".to_string(),
                    },
                    ..Default::default()
                },
                status: Some(BackupStatus {
                    phase: Some("completed".to_string()),
                    stopped_at: Some("2023-09-19T21:11:00Z".to_string()),
                    ..Default::default()
                }),
            },
            Backup {
                metadata: ObjectMeta {
                    name: Some("backup-4".to_string()),
                    namespace: Some("test".to_string()),
                    ..Default::default()
                },
                spec: BackupSpec {
                    cluster: BackupCluster {
                        name: "backup-4".to_string(),
                    },
                    ..Default::default()
                },
                status: Some(BackupStatus {
                    phase: Some("failed".to_string()),
                    stopped_at: Some("2023-09-19T21:11:00Z".to_string()),
                    ..Default::default()
                }),
            },
        ];

        let oldest_backup_time = coredb.process_backups(backup_list);

        let expected_time = NaiveDate::from_ymd_opt(2023, 9, 18)
            .and_then(|date| date.and_hms_opt(22, 12, 0))
            .map(|naive_dt| DateTime::from_naive_utc_and_offset(naive_dt, Utc));

        assert_eq!(oldest_backup_time, expected_time);
    }

    #[tokio::test]
    async fn test_process_backups_no_backup() {
        let coredb = CoreDB::test();

        // An empty list to simulate no Backups
        let backup_list: Vec<Backup> = vec![];

        let oldest_backup_time = coredb.process_backups(backup_list);

        // We expect None since there are no Backups
        assert_eq!(oldest_backup_time, None);
    }

    #[test]
    fn test_create_volume_snapshot_patch_enabled() {
        let cfg = Config {
            enable_volume_snapshot: true,
            ..Config::default()
        };

        let expected_volume_snapshot = VolumeSnapshot {
            enabled: true,
            snapshot_class: Some(VOLUME_SNAPSHOT_CLASS_NAME.to_string()),
        };

        let actual_patch = create_volume_snapshot_patch(&cfg);

        // Deserialize the actual_patch into a VolumeSnapshot instance
        let actual_volume_snapshot: VolumeSnapshot =
            serde_json::from_value(actual_patch["spec"]["backup"]["volumeSnapshot"].clone())
                .expect("Failed to deserialize actual_patch into VolumeSnapshot");

        assert_eq!(actual_volume_snapshot, expected_volume_snapshot);
    }

    #[test]
    fn test_create_volume_snapshot_patch_disabled() {
        let cfg = Config {
            enable_volume_snapshot: false,
            ..Config::default()
        };

        let expected_volume_snapshot = VolumeSnapshot {
            enabled: false,
            snapshot_class: None,
        };

        let actual_patch = create_volume_snapshot_patch(&cfg);

        // Deserialize the actual_patch into a VolumeSnapshot instance
        let actual_volume_snapshot: VolumeSnapshot =
            serde_json::from_value(actual_patch["spec"]["backup"]["volumeSnapshot"].clone())
                .expect("Failed to deserialize actual_patch into VolumeSnapshot");

        assert_eq!(actual_volume_snapshot, expected_volume_snapshot);
    }

    #[test]
    fn test_is_volume_snapshot_update_needed() {
        let volume_snapshot_enabled = Some(VolumeSnapshot {
            enabled: true,
            snapshot_class: Some(VOLUME_SNAPSHOT_CLASS_NAME.to_string()),
        });
        let volume_snapshot_disabled = Some(VolumeSnapshot {
            enabled: false,
            snapshot_class: None,
        });

        // Test cases where no update is needed
        assert!(!is_volume_snapshot_update_needed(
            volume_snapshot_enabled.as_ref(),
            true
        ));
        assert!(!is_volume_snapshot_update_needed(
            volume_snapshot_disabled.as_ref(),
            false
        ));
        assert!(!is_volume_snapshot_update_needed(None, false));

        // Test cases where an update is needed
        assert!(is_volume_snapshot_update_needed(
            volume_snapshot_enabled.as_ref(),
            false
        ));
        assert!(is_volume_snapshot_update_needed(
            volume_snapshot_disabled.as_ref(),
            true
        ));
        assert!(is_volume_snapshot_update_needed(None, true));
    }

    // Test the error_policy function, we need to mock the ctx and cdb to mimic a 429 error code
    use crate::{error_policy, Error};
    use futures::pin_mut;
    use http::{Request, Response, StatusCode};
    use hyper::Body;
    use k8s_openapi::api::core::v1::Pod;
    use kube::{api::Api, Client};
    use serde_json::json;
    use tower_test::mock;

    #[tokio::test]
    async fn test_error_policy_429() {
        // setup a test CoreDB object
        let coredb = CoreDB::test();

        // mock the Kubernetes client and setup Context
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let client = Client::new(mock_service, "default".to_string());
        let ctx = Arc::new(Context {
            client: client.clone(),
            metrics: Default::default(),
            diagnostics: Default::default(),
        });

        // setup the mock response 429 too many requests
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            if let Some((_request, send)) = handle.next_request().await {
                // We don't check the specifics of the request here, focusing on the response
                send.send_response(
                    Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .body(Body::from(
                            json!({
                                "kind": "Status",
                                "apiVersion": "v1",
                                "metadata": {},
                                "status": "Failure",
                                "message": "Too Many Requests",
                                "reason": "TooManyRequests",
                                "code": 429
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                );
            }
        });

        // Setup call to kubernetes api Pod
        let pod_api: Api<Pod> = Api::namespaced(ctx.client.clone(), "default");
        let err = pod_api.get("test-pod").await.err().unwrap();

        // Convert the KubeError into your custom error type as it would in your controller logic
        let custom_error = Error::from(err);

        // Now we simulate calling the error_policy function with this error
        let action = error_policy(Arc::new(coredb), &custom_error, ctx);
        let action_str = format!("{:?}", action);

        println!("Action: {:?}", action);

        // Use regular expressions to extract the duration from the action string
        let re = regex::Regex::new(r"requeue_after: Some\((\d+)s\)").unwrap();
        if let Some(captures) = re.captures(&action_str) {
            let duration_secs = captures[1].parse::<u64>().unwrap();
            assert!((60..=180).contains(&duration_secs));
        } else {
            panic!("Unexpected action format: {}", action_str);
        }

        spawned.await.unwrap();
    }

    #[tokio::test]
    async fn test_error_policy_non_429() {
        // setup a test CoreDB object
        let coredb = CoreDB::test();

        // mock the Kubernetes client and setup Context
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let client = Client::new(mock_service, "default".to_string());
        let ctx = Arc::new(Context {
            client: client.clone(),
            metrics: Default::default(),
            diagnostics: Default::default(),
        });

        // setup the mock response 404 Not Found
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            if let Some((_request, send)) = handle.next_request().await {
                send.send_response(
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Body::from(
                            json!({
                                "kind": "Status",
                                "apiVersion": "v1",
                                "metadata": {},
                                "status": "Failure",
                                "message": "Not Found",
                                "reason": "NotFound",
                                "code": 404
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                );
            }
        });

        // Setup call to kubernetes api Pod
        let pod_api: Api<Pod> = Api::namespaced(ctx.client.clone(), "default");
        let err = pod_api.get("test-pod").await.err().unwrap();

        // Convert the KubeError into your custom error type as it would in your controller logic
        let custom_error = Error::from(err);

        // Now we simulate calling the error_policy function with this error
        let action = error_policy(Arc::new(coredb), &custom_error, ctx);
        let action_str = format!("{:?}", action);

        println!("Action: {:?}", action);

        // Assert that the action is a requeue with a duration of 5 minutes (300 seconds)
        let re = regex::Regex::new(r"requeue_after: Some\((\d+)s\)").unwrap();
        if let Some(captures) = re.captures(&action_str) {
            let duration_secs = captures[1].parse::<u64>().unwrap();
            assert_eq!(duration_secs, 300);
        } else {
            panic!("Unexpected action format: {}", action_str);
        }

        spawned.await.unwrap();
    }
}
