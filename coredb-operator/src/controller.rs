use crate::{telemetry, Error, Metrics, Result};
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, StatefulSet, StatefulSetSpec};
use k8s_openapi::api::core::v1::{Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use std::collections::BTreeMap;
use kube::api::{DeleteParams, ObjectMeta, PostParams};
use chrono::{DateTime, Utc};
use futures::{future::BoxFuture, FutureExt, StreamExt};
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
    },
    CustomResource, Resource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::*;

pub static COREDB_FINALIZER: &str = "coredbs.kube.rs";

/// Generate the Kubernetes wrapper struct `CoreDB` from our Spec and Status struct
///
/// This provides a hook for generating the CRD yaml (in crdgen.rs)
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Default))]
#[kube(kind = "CoreDB", group = "kube.rs", version = "v1", namespaced)]
#[kube(status = "CoreDBStatus", shortname = "cdb")]
pub struct CoreDBSpec {
    pub replicas: i32,
    pub hide: bool,
}
/// The status object of `CoreDB`
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct CoreDBStatus {
    pub hidden: bool,
}

impl CoreDB {
    fn was_hidden(&self) -> bool {
        self.status.as_ref().map(|s| s.hidden).unwrap_or(false)
    }
}

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
            Finalizer::Apply(cdb) => cdb.reconcile(ctx.clone()).await,
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
    async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action> {
        let client = ctx.client.clone();
        let recorder = ctx.diagnostics.read().await.recorder(client.clone(), self);
        let ns = self.namespace().unwrap();
        let name = self.name_any();
        let coredbs: Api<CoreDB> = Api::namespaced(client, &ns);

        let should_hide = self.spec.hide;
        if !self.was_hidden() && should_hide {
            // only send event the first time
            recorder
                .publish(Event {
                    type_: EventType::Normal,
                    reason: "HiddenCoreDB".into(),
                    note: Some(format!("Hiding `{name}`")),
                    action: "Reconciling".into(),
                    secondary: None,
                })
                .await
                .map_err(Error::KubeError)?;
        }
        // if name == "illegal" {
        //     return Err(Error::IllegalCoreDB); // error names show up in metrics
        // }
        // always overwrite status object with what we saw
        let new_status = Patch::Apply(json!({
            "apiVersion": "kube.rs/v1",
            "kind": "CoreDB",
            "status": CoreDBStatus {
                hidden: should_hide,
            }
        }));
        let ps = PatchParams::apply("cntrlr").force();
        let _o = coredbs
            .patch_status(&name, &ps, &new_status)
            .await
            .map_err(Error::KubeError)?;
        // create statefulset
        self.create_sts(ctx)
            .await
            .expect("error creating statefulset");
        // If no events were received, check back every 5 minutes
        Ok(Action::requeue(Duration::from_secs(5 * 60)))
    }

    async fn create_sts(&self, ctx: Arc<Context>) -> Result<Action> {
        let client = ctx.client.clone();
        let ns = self.namespace().unwrap();
        let name = self.name_any();
        let mut labels: BTreeMap<String, String> = BTreeMap::new();
        let mut env: BTreeMap<String, String> = BTreeMap::new();
        let sts_api: Api<StatefulSet> = Api::namespaced(client, &ns);
        labels.insert("app".to_owned(), name.to_owned());
        let sts: StatefulSet = StatefulSet {
            metadata: ObjectMeta {
                name: Some(name.to_owned()),
                namespace: Some(ns.to_owned()),
                labels: Some(labels.clone()),
                ..ObjectMeta::default()
            },
            spec: Some(StatefulSetSpec {
                replicas: Some(self.spec.replicas),
                selector: LabelSelector {
                    match_expressions: None,
                    match_labels: Some(labels.clone()),
                },
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            env: Option::from(vec![EnvVar {
                                name: "POSTGRES_PASSWORD".parse().unwrap(),
                                value: Some("password".parse().unwrap()),
                                value_from: None
                            }]),
                            name: name.to_owned(),
                            image: Some("docker.io/postgres:15".to_owned()),
                            ports: Some(vec![ContainerPort {
                                container_port: 8080,
                                ..ContainerPort::default()
                            }]),
                            ..Container::default()
                        }],
                        ..PodSpec::default()
                    }),
                    metadata: Some(ObjectMeta {
                        labels: Some(labels),
                        ..ObjectMeta::default()
                    }),
                },
                ..StatefulSetSpec::default()
            }),
            ..StatefulSet::default()
        };

        // Create the statefulset defined above
        sts_api
            .create(&PostParams::default(), &sts)
            .await
            .map_err(Error::KubeError)?;

        // If no events were received, check back every 5 minutes
        Ok(Action::requeue(Duration::from_secs(5 * 60)))
    }

    async fn delete_sts(&self, client: Client, name: &str, namespace: &str) -> Result<(), Error> {
        let sts: Api<StatefulSet> = Api::namespaced(client, namespace);
        sts
            .delete(name, &DeleteParams::default())
            .await
            .map_err(Error::KubeError)?;
        Ok(())
    }

    // Finalizer cleanup (the object was deleted, ensure nothing is orphaned)
    async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action> {
        let client = ctx.client.clone();
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
        self.delete_sts(client, &self.name(), &self.namespace().unwrap())
            .await
            .expect("error deleting statefulset");
        Ok(Action::await_change())
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
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .boxed();
    (controller, state)
}
