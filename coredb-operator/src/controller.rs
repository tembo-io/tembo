use crate::{telemetry, Error, Metrics, Result};
use chrono::{DateTime, Utc};
use futures::{
    future::{BoxFuture, FutureExt},
    stream::StreamExt,
};

use crate::{
    exec::{ExecCommand, ExecOutput},
    psql::{PsqlCommand, PsqlOutput},
    service::reconcile_svc,
    statefulset::{reconcile_sts, stateful_set_from_cdb},
};
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
    },
    Resource,
};

use crate::{
    apis::coredb_types::{CoreDB, CoreDBStatus},
    extensions::{reconcile_extensions, Extension},
    postgres_exporter_role::create_postgres_exporter_role,
    secret::reconcile_secret,
};
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::runtime::wait::Condition;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::*;

pub static COREDB_FINALIZER: &str = "coredbs.coredb.io";

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
    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();
    let ns = cdb.namespace().unwrap(); // cdb is namespace scoped
    let coredbs: Api<CoreDB> = Api::namespaced(ctx.client.clone(), &ns);

    info!("Reconciling CoreDB \"{}\" in {}", cdb.name_any(), ns);
    finalizer(&coredbs, COREDB_FINALIZER, cdb, |event| async {
        match event {
            Finalizer::Apply(cdb) => match cdb.reconcile(ctx.clone()).await {
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

impl CoreDB {
    // Reconcile (for non-finalizer related changes)
    async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action, Action> {
        let client = ctx.client.clone();
        let _recorder = ctx.diagnostics.read().await.recorder(client.clone(), self);
        let ns = self.namespace().unwrap();
        let name = self.name_any();
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &ns);

        // reconcile secret
        reconcile_secret(self, ctx.clone()).await.map_err(|e| {
            error!("Error reconciling secret: {:?}", e);
            Action::requeue(Duration::from_secs(10))
        })?;

        // reconcile statefulset
        reconcile_sts(self, ctx.clone()).await.map_err(|e| {
            error!("Error reconciling statefulset: {:?}", e);
            Action::requeue(Duration::from_secs(10))
        })?;

        // reconcile service
        reconcile_svc(self, ctx.clone()).await.map_err(|e| {
            error!("Error reconciling service: {:?}", e);
            Action::requeue(Duration::from_secs(10))
        })?;

        let new_status = match self.spec.stop {
            false => {
                let primary_pod = self.primary_pod(ctx.client.clone()).await;
                if primary_pod.is_err() {
                    debug!("Did not find primary pod");
                    return Ok(Action::requeue(Duration::from_secs(1)));
                }
                let primary_pod = primary_pod.unwrap();

                if !is_postgres_ready().matches_object(Some(&primary_pod)) {
                    debug!("Postgres is not ready");
                    return Ok(Action::requeue(Duration::from_secs(1)));
                }
                // creating exporter role is pre-requisite to the postgres pod becoming "ready"
                create_postgres_exporter_role(self, ctx.clone())
                    .await
                    .map_err(|e| {
                        error!(
                            "Error creating postgres_exporter on CoreDB {}, {}",
                            self.metadata.name.clone().unwrap(),
                            e
                        );
                        Action::requeue(Duration::from_secs(5))
                    })?;

                if !is_pod_ready().matches_object(Some(&primary_pod)) {
                    debug!("Did not find primary pod");
                    return Ok(Action::requeue(Duration::from_secs(1)));
                }

                let mut extensions: Vec<Extension> =
                    reconcile_extensions(self, ctx.clone()).await.map_err(|e| {
                        error!("Error reconciling extensions: {:?}", e);
                        Action::requeue(Duration::from_secs(10))
                    })?;

                // must be sorted same, else reconcile will trigger again
                extensions.sort_by_key(|e| e.name.clone());
                CoreDBStatus {
                    running: true,
                    storage: self.spec.storage.clone(),
                    sharedirStorage: self.spec.sharedirStorage.clone(),
                    pkglibdirStorage: self.spec.pkglibdirStorage.clone(),
                    extensions: Some(extensions),
                }
            }
            true => CoreDBStatus {
                running: false,
                storage: self.spec.storage.clone(),
                sharedirStorage: self.spec.sharedirStorage.clone(),
                pkglibdirStorage: self.spec.pkglibdirStorage.clone(),
                extensions: Some(self.status.as_ref().unwrap().extensions.clone().unwrap()),
            },
        };

        let patch_status = Patch::Apply(json!({
            "apiVersion": "coredb.io/v1alpha1",
            "kind": "CoreDB",
            "status": new_status
        }));

        let ps = PatchParams::apply("cntrlr").force();
        let _o = coredbs
            .patch_status(&name, &ps, &patch_status)
            .await
            .map_err(|e| {
                error!("Error updating CoreDB status: {:?}", e);
                Action::requeue(Duration::from_secs(10))
            })?;

        // If no events were received, check back every minute
        Ok(Action::requeue(Duration::from_secs(60)))
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

    pub async fn primary_pod(&self, client: Client) -> Result<Pod, Error> {
        let sts = stateful_set_from_cdb(self);
        let sts_name = sts.metadata.name.unwrap();
        let sts_namespace = sts.metadata.namespace.unwrap();
        let label_selector = format!("statefulset={sts_name}");
        let list_params = ListParams::default().labels(&label_selector);
        let pods: Api<Pod> = Api::namespaced(client, &sts_namespace);
        let pods = pods.list(&list_params);
        // Return an error if the query fails
        let pod_list = pods.await.map_err(Error::KubeError)?;
        // Return an error if the list is empty
        if pod_list.items.is_empty() {
            return Err(Error::KubeError(kube::Error::Api(kube::error::ErrorResponse {
                status: "404".to_string(),
                message: "No pods found".to_string(),
                reason: "Not Found".to_string(),
                code: 404,
            })));
        }
        let primary = pod_list.items[0].clone();
        Ok(primary)
    }

    pub async fn psql(
        &self,
        command: String,
        database: String,
        client: Client,
    ) -> Result<PsqlOutput, kube::Error> {
        let pod_name = self
            .primary_pod(client.clone())
            .await
            .unwrap()
            .metadata
            .name
            .unwrap();

        PsqlCommand::new(
            pod_name,
            self.metadata.namespace.clone().unwrap(),
            command,
            database,
            client,
        )
        .execute()
        .await
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
pub async fn init(client: Client) -> (BoxFuture<'static, ()>, State) {
    let state = State::default();
    let cdb = Api::<CoreDB>::all(client.clone());
    if let Err(e) = cdb.list(&ListParams::default().limit(1)).await {
        error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }
    let controller = Controller::new(cdb, ListParams::default())
        .run(reconcile, error_policy, state.create_context(client))
        .filter_map(|x| async move { Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .boxed();
    (controller, state)
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
