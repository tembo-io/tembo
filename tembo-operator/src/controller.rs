use chrono::{DateTime, Utc};
use futures::stream::StreamExt;

use crate::{
    apis::{
        coredb_types::{CoreDB, CoreDBStatus},
        postgres_parameters::reconcile_pg_parameters_configmap,
    },
    cloudnativepg::cnpg::{cnpg_cluster_from_cdb, reconcile_cnpg, reconcile_cnpg_scheduled_backup},
    config::Config,
    deployment_postgres_exporter::reconcile_prometheus_exporter,
    exec::{ExecCommand, ExecOutput},
    extensions::{reconcile_extensions, Extension},
    ingress::reconcile_postgres_ing_route_tcp,
    postgres_exporter::{create_postgres_exporter_role, reconcile_prom_configmap},
    psql::{PsqlCommand, PsqlOutput},
    rbac::reconcile_rbac,
    secret::{reconcile_postgres_exporter_secret, reconcile_secret, PrometheusExporterSecretData},
    service::reconcile_svc,
    telemetry, Error, Metrics, Result,
};
use k8s_openapi::{
    api::{
        core::v1::{Namespace, Pod},
        rbac::v1::PolicyRule,
    },
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

#[instrument(skip(ctx, cdb), fields(trace_id))]
async fn reconcile(cdb: Arc<CoreDB>, ctx: Arc<Context>) -> Result<Action> {
    let cfg = Config::default();
    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();
    let ns = cdb.namespace().unwrap(); // cdb is namespace scoped
    let coredbs: Api<CoreDB> = Api::namespaced(ctx.client.clone(), &ns);
    // Get metadata for the CoreDB object
    let metadata = cdb.meta().clone();
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

fn error_policy(cdb: Arc<CoreDB>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(&cdb, error);
    Action::requeue(Duration::from_secs(5 * 60))
}

// Create role policy rulesets
async fn create_policy_rules(cdb: &CoreDB) -> Vec<PolicyRule> {
    vec![
        // This policy allows get, list, watch access to the coredb resource
        PolicyRule {
            api_groups: Some(vec!["coredb.io".to_owned()]),
            resource_names: Some(vec![cdb.name_any()]),
            resources: Some(vec!["coredbs".to_owned()]),
            verbs: vec!["get".to_string(), "list".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
        // This policy allows get, patch, update, watch access to the coredb/status resource
        PolicyRule {
            api_groups: Some(vec!["coredb.io".to_owned()]),
            resource_names: Some(vec![cdb.name_any()]),
            resources: Some(vec!["coredbs/status".to_owned()]),
            verbs: vec![
                "get".to_string(),
                "patch".to_string(),
                "update".to_string(),
                "watch".to_string(),
            ],
            ..PolicyRule::default()
        },
        // This policy allows get, watch access to a secret in the namespace
        PolicyRule {
            api_groups: Some(vec!["".to_owned()]),
            resource_names: Some(vec![format!("{}-connection", cdb.name_any())]),
            resources: Some(vec!["secrets".to_owned()]),
            verbs: vec!["get".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
        // This policy for now is specifically open for all configmaps in the namespace
        // We currently do not have any configmaps
        PolicyRule {
            api_groups: Some(vec!["".to_owned()]),
            resources: Some(vec!["configmaps".to_owned()]),
            verbs: vec!["get".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
    ]
}

impl CoreDB {
    pub(crate) async fn cnpg_enabled(&self, ctx: Arc<Context>) -> bool {
        // We will migrate databases by applying this label manually to the namespace
        let cnpg_enabled_label = "tembo-pod-init.tembo.io/watch";

        let client = ctx.client.clone();
        // Get labels of the current namespace
        let ns_api: Api<Namespace> = Api::all(client.clone());
        let ns = self.namespace().unwrap();
        let ns_labels = ns_api
            .get(&ns)
            .await
            .unwrap_or_default()
            .metadata
            .labels
            .unwrap_or_default();

        let enabled_value = ns_labels.get(&String::from(cnpg_enabled_label));
        if enabled_value.is_some() {
            let enabled = enabled_value.expect("We already checked this is_some") == "true";
            return enabled;
        }
        false
    }

    // Reconcile (for non-finalizer related changes)
    async fn reconcile(&self, ctx: Arc<Context>, cfg: &Config) -> Result<Action, Action> {
        let client = ctx.client.clone();
        let _recorder = ctx.diagnostics.read().await.recorder(client.clone(), self);
        let ns = self.namespace().unwrap();
        let name = self.name_any();
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &ns);

        let cnpg_enabled = self.cnpg_enabled(ctx.clone()).await;
        match std::env::var("DATA_PLANE_BASEDOMAIN") {
            Ok(basedomain) => {
                debug!(
                    "DATA_PLANE_BASEDOMAIN is set to {}, reconciling ingress route tcp",
                    basedomain
                );
                let service_name_read_write = match cnpg_enabled {
                    // When CNPG is enabled, we use the CNPG service name
                    true => format!("{}-rw", self.name_any().as_str()),
                    false => self.name_any().as_str().to_string(),
                };
                reconcile_postgres_ing_route_tcp(
                    self,
                    ctx.clone(),
                    self.name_any().as_str(),
                    basedomain.as_str(),
                    ns.as_str(),
                    service_name_read_write.as_str(),
                    IntOrString::Int(5432),
                )
                .await
                .map_err(|e| {
                    error!("Error reconciling postgres ingress route: {:?}", e);
                    // For unexpected errors, we should requeue for several minutes at least,
                    // for expected, "waiting" type of requeuing, those should be shorter, just a few seconds.
                    // IngressRouteTCP does not have expected errors during reconciliation.
                    Action::requeue(Duration::from_secs(300))
                })?;
            }
            Err(_e) => {
                warn!("DATA_PLANE_BASEDOMAIN is not set, skipping reconciliation of IngressRouteTCP");
            }
        };

        // create/update configmap when postgres exporter enabled
        if self.spec.postgresExporterEnabled {
            debug!("Reconciling prometheus configmap");
            reconcile_prom_configmap(self, client.clone(), &ns)
                .await
                .map_err(|e| {
                    error!("Error reconciling prometheus configmap: {:?}", e);
                    Action::requeue(Duration::from_secs(300))
                })?;
        }

        // reconcile service account, role, and role binding
        reconcile_rbac(self, ctx.clone(), None, create_policy_rules(self).await)
            .await
            .map_err(|e| {
                error!("Error reconciling service account: {:?}", e);
                Action::requeue(Duration::from_secs(300))
            })?;

        // reconcile secret
        debug!("Reconciling secret");
        reconcile_secret(self, ctx.clone()).await.map_err(|e| {
            error!("Error reconciling secret: {:?}", e);
            Action::requeue(Duration::from_secs(300))
        })?;

        // reconcile postgres exporter secret
        let secret_data: Option<PrometheusExporterSecretData> = if self.spec.postgresExporterEnabled {
            let result = reconcile_postgres_exporter_secret(self, ctx.clone())
                .await
                .map_err(|e| {
                    error!("Error reconciling postgres exporter secret: {:?}", e);
                    Action::requeue(Duration::from_secs(300))
                })?;

            match result {
                Some(data) => Some(data),
                None => {
                    warn!("Secret already exists, no new password is generated");
                    None
                }
            }
        } else {
            None
        };

        // handle postgres configs
        debug!("Reconciling postgres configmap");
        reconcile_pg_parameters_configmap(self, client.clone(), &ns)
            .await
            .map_err(|e| {
                error!("Error reconciling postgres configmap: {:?}", e);
                Action::requeue(Duration::from_secs(300))
            })?;

        if cnpg_enabled {
            reconcile_cnpg(self, ctx.clone()).await?;
            if cfg.enable_backup {
                reconcile_cnpg_scheduled_backup(self, ctx.clone()).await?;
            }
        }

        // reconcile prometheus exporter deployment if enabled
        if self.spec.postgresExporterEnabled {
            debug!("Reconciling prometheus exporter deployment");
            reconcile_prometheus_exporter(self, ctx.clone(), cnpg_enabled)
                .await
                .map_err(|e| {
                    error!("Error reconciling prometheus exporter deployment: {:?}", e);
                    Action::requeue(Duration::from_secs(300))
                })?;
        };

        // reconcile service
        debug!("Reconciling service");
        reconcile_svc(self, ctx.clone()).await.map_err(|e| {
            error!("Error reconciling service: {:?}", e);
            Action::requeue(Duration::from_secs(300))
        })?;

        let new_status = match self.spec.stop {
            false => {
                let primary_pod_cnpg = self.primary_pod_cnpg(ctx.client.clone()).await?;

                if !is_postgres_ready().matches_object(Some(&primary_pod_cnpg)) {
                    debug!(
                        "Did not find postgres ready {}, waiting a short period",
                        self.name_any()
                    );
                    return Ok(Action::requeue(Duration::from_secs(5)));
                }

                let extensions: Vec<Extension> =
                    reconcile_extensions(self, ctx.clone(), &coredbs, &name).await?;

                create_postgres_exporter_role(self, ctx.clone(), secret_data).await?;

                CoreDBStatus {
                    running: true,
                    extensionsUpdating: false,
                    storage: self.spec.storage.clone(),
                    sharedirStorage: self.spec.sharedirStorage.clone(),
                    pkglibdirStorage: self.spec.pkglibdirStorage.clone(),
                    extensions: Some(extensions),
                }
            }
            true => CoreDBStatus {
                running: false,
                extensionsUpdating: false,
                storage: self.spec.storage.clone(),
                sharedirStorage: self.spec.sharedirStorage.clone(),
                pkglibdirStorage: self.spec.pkglibdirStorage.clone(),
                extensions: self.status.clone().and_then(|f| f.extensions),
            },
        };

        let patch_status = json!({
            "apiVersion": "coredb.io/v1alpha1",
            "kind": "CoreDB",
            "status": new_status
        });

        patch_cdb_status_force(&coredbs, &name, patch_status).await?;

        // Check back every 5 minutes
        Ok(Action::requeue(Duration::from_secs(300)))
    }

    // Finalizer cleanup (the object was deleted, ensure nothing is orphaned)
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
        let recorder = ctx.diagnostics.read().await.recorder(ctx.client.clone(), self);
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

    pub async fn primary_pod_cnpg(&self, client: Client) -> Result<Pod, Action> {
        let cluster = cnpg_cluster_from_cdb(self);
        let cluster_name = cluster
            .metadata
            .name
            .expect("CNPG Cluster should always have a name");
        let namespace = self
            .metadata
            .namespace
            .clone()
            .expect("Operator should always be namespaced");
        let cluster_selector = format!("cnpg.io/cluster={cluster_name}");
        let role_selector = "role=primary".to_string();
        let list_params = ListParams::default()
            .labels(&cluster_selector)
            .labels(&role_selector);
        let pods: Api<Pod> = Api::namespaced(client, &namespace);
        let pods = pods.list(&list_params);
        // Return an error if the query fails
        let pod_list = pods.await.map_err(|_e| {
            // It is not expected to fail the query to the pods API
            error!("Failed to query for CNPG primary pod of {}", &self.name_any());
            Action::requeue(Duration::from_secs(300))
        })?;
        // Return an error if the list is empty
        if pod_list.items.is_empty() {
            // It's expected to sometimes be empty, we should retry after a short duration
            warn!("Failed to find CNPG primary pod of {}, this can be expected if the pod is restarting for some reason", &self.name_any());
            return Err(Action::requeue(Duration::from_secs(5)));
        }
        let primary = pod_list.items[0].clone();
        Ok(primary)
    }

    pub async fn psql(
        &self,
        command: String,
        database: String,
        context: Arc<Context>,
    ) -> Result<PsqlOutput, Action> {
        let client = context.client.clone();
        let _cnpg_enabled = self.cnpg_enabled(context.clone()).await;

        let pod_name_cnpg = self
            .primary_pod_cnpg(client.clone())
            .await?
            .metadata
            .name
            .expect("All pods should have a name");

        let cnpg_psql_command = PsqlCommand::new(
            pod_name_cnpg.clone(),
            self.metadata.namespace.clone().unwrap(),
            command,
            database,
            context,
        );
        debug!("Running exec command in {}", pod_name_cnpg);
        let cnpg_exec = cnpg_psql_command.execute();
        cnpg_exec.await.map_err(|_e| {
            warn!("Failed executing command in primary pod of {}", &self.name_any());
            Action::requeue(Duration::from_secs(30))
        })
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

pub async fn patch_cdb_status_force(
    cdb: &Api<CoreDB>,
    name: &str,
    patch: serde_json::Value,
) -> Result<(), Action> {
    let ps = PatchParams::apply("cntrlr").force();
    let patch_status = Patch::Apply(patch);
    let _o = cdb.patch_status(name, &ps, &patch_status).await.map_err(|e| {
        error!("Error updating CoreDB status: {:?}", e);
        Action::requeue(Duration::from_secs(10))
    })?;
    Ok(())
}

pub async fn patch_cdb_status_merge(
    cdb: &Api<CoreDB>,
    name: &str,
    patch: serde_json::Value,
) -> Result<(), Action> {
    let pp = PatchParams::default();
    let patch_status = Patch::Merge(patch);
    let _o = cdb.patch_status(name, &pp, &patch_status).await.map_err(|e| {
        error!("Error updating CoreDB status: {:?}", e);
        Action::requeue(Duration::from_secs(10))
    })?;
    Ok(())
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
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "coredb-controller".into(),
        }
    }
}
impl Diagnostics {
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

    let docs = Api::<CoreDB>::all(client.clone());
    if let Err(e) = docs.list(&ListParams::default().limit(1)).await {
        error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }
    Controller::new(docs, watcherConfig::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.create_context(client))
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

// Tests rely on fixtures.rs
#[cfg(test)]
mod test {
    use super::{reconcile, Context, CoreDB};
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
    async fn test_patches_coredb() {
        let (testctx, fakeserver, _) = Context::test();
        let coredb = CoreDB::test().finalized();
        fakeserver.handle_coredb_patch(&coredb);
        let res = reconcile(Arc::new(coredb), testctx).await;
        assert!(res.is_ok(), "finalized coredb succeeds in its reconciler");
    }
}
