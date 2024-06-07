//! Helper methods only available for tests
use crate::{
    apis::coredb_types::{CoreDB, CoreDBSpec, CoreDBStatus},
    Context, Metrics, COREDB_FINALIZER,
};
use assert_json_diff::assert_json_include;
use futures::pin_mut;
use http::{Request, Response};
use k8s_openapi::api::core::v1::{Pod, Secret};
use kube::{
    api::ObjectList, api::ObjectMeta, client::Body, core::TypeMeta, Client, Resource, ResourceExt,
};
use prometheus::Registry;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tower_test::mock::{self, Handle};

impl CoreDB {
    /// A normal test CoreDB
    pub fn test() -> Self {
        let mut d = CoreDB::new("testdb", CoreDBSpec::default());
        d.meta_mut().namespace = Some("testns".into());
        d.meta_mut().uid = Some("752d59ef-2671-4890-9feb-0097459b18c8".into());
        d.spec.replicas = 1;
        // Need to figure out how to mock websocket
        // in order to unit test a feature using kube exec
        d.spec.postgresExporterEnabled = false;
        d
    }

    /// Modify a coredb to have the expected finalizer
    pub fn finalized(mut self) -> Self {
        self.finalizers_mut().push(COREDB_FINALIZER.to_string());
        self
    }

    /// Modify a coredb to have an expected status
    pub fn with_status(mut self, status: CoreDBStatus) -> Self {
        self.status = Some(status);
        self
    }
}
pub struct ApiServerVerifier(Handle<Request<Body>, Response<Body>>);

/// Create a responder + verifier object that deals with the main reconcile scenarios
///
impl ApiServerVerifier {
    pub fn handle_finalizer_creation(self, coredb_: &CoreDB) -> JoinHandle<()> {
        let handle = self.0;
        let coredb = coredb_.clone();
        tokio::spawn(async move {
            pin_mut!(handle);
            let (request, send) = handle.next_request().await.expect("service not called");
            // We expect a json patch to the specified coredb adding our finalizer
            assert_eq!(request.method(), http::Method::PATCH);
            assert_eq!(
                request.uri().to_string(),
                format!(
                    "/apis/coredb.io/v1alpha1/namespaces/testns/coredbs/{}?",
                    coredb.name_any()
                )
            );
            let expected_patch = serde_json::json!([
                { "op": "test", "path": "/metadata/finalizers", "value": null },
                { "op": "add", "path": "/metadata/finalizers", "value": vec![COREDB_FINALIZER] }
            ]);
            let req_body = request.into_body().collect_bytes().await.unwrap();
            let runtime_patch: serde_json::Value =
                serde_json::from_slice(&req_body).expect("valid coredb from runtime");
            assert_json_include!(actual: runtime_patch, expected: expected_patch);

            let response = serde_json::to_vec(&coredb.finalized()).unwrap(); // respond as the apiserver would have
            send.send_response(Response::builder().body(Body::from(response)).unwrap());
        })
    }

    pub fn handle_coredb_patch(self, coredb_: &CoreDB) -> JoinHandle<()> {
        let handle = self.0;
        let coredb = coredb_.clone();
        tokio::spawn(async move {
            pin_mut!(handle);
            // After the PATCH to CoreDB, we expect a GET on Secrets
            let (request, send) = handle
                .next_request()
                .await
                .expect("Kube API called to GET Secret");
            assert_eq!(request.method(), http::Method::GET);
            assert_eq!(
                request.uri().to_string(),
                format!("/api/v1/namespaces/testns/secrets?&labelSelector=app%3Dcoredb")
            );

            // We need to send an empty ObjectList<Secret> back as our response
            let obj: ObjectList<Secret> = ObjectList {
                metadata: Default::default(),
                items: vec![],
                types: TypeMeta {
                    kind: "Secret".to_string(),
                    api_version: "v1".to_string(),
                },
            };
            let response = serde_json::to_vec(&obj).unwrap();
            send.send_response(Response::builder().body(Body::from(response)).unwrap());

            // After the GET on Secrets, we expect a PATCH to Secret
            let (request, send) = handle
                .next_request()
                .await
                .expect("Kube API called to PATCH Secret");
            assert_eq!(request.method(), http::Method::PATCH);
            assert_eq!(
                request.uri().to_string(),
                format!(
                    "/api/v1/namespaces/testns/secrets/testdb-connection?&force=true&fieldManager=cntrlr"
                )
            );
            send.send_response(Response::builder().body(request.into_body()).unwrap());

            // After the PATCH to Secret, we expect a PATCH to StatefulSet
            let (request, send) = handle
                .next_request()
                .await
                .expect("Kube API called to PATCH StatefulSet");
            assert_eq!(request.method(), http::Method::PATCH);
            assert_eq!(
                request.uri().to_string(),
                format!(
                    "/apis/apps/v1/namespaces/testns/statefulsets/testdb?&force=true&fieldManager=cntrlr"
                )
            );
            send.send_response(Response::builder().body(request.into_body()).unwrap());

            // After the PATCH to StatefulSet, we expect a PATCH to Service
            let (request, send) = handle
                .next_request()
                .await
                .expect("Kube API called to PATCH Service");
            assert_eq!(request.method(), http::Method::PATCH);
            assert_eq!(
                request.uri().to_string(),
                format!(
                    "/api/v1/namespaces/testns/services/testdb?&force=true&fieldManager=cntrlr"
                )
            );
            send.send_response(Response::builder().body(request.into_body()).unwrap());

            // After the PATCH to Service, we expect a GET to Pods
            let (request, send) = handle
                .next_request()
                .await
                .expect("Kube API called to GET Pods");
            assert_eq!(request.method(), http::Method::GET);
            assert_eq!(
                request.uri().to_string(),
                format!("/api/v1/namespaces/testns/pods?&labelSelector=statefulset%3Dtestdb")
            );

            // We need to send an ObjectList<Pod> back as our response
            let pod: Pod = Pod {
                metadata: ObjectMeta {
                    name: Some("testdb-0".to_string()),
                    namespace: Some("testns".to_string()),
                    ..ObjectMeta::default()
                },
                ..Pod::default()
            };
            let obj: ObjectList<Pod> = ObjectList {
                metadata: Default::default(),
                items: vec![pod],
                types: TypeMeta {
                    kind: "Pod".to_string(),
                    api_version: "v1".to_string(),
                },
            };
            let response = serde_json::to_vec(&obj).unwrap();
            send.send_response(Response::builder().body(Body::from(response)).unwrap());

            // expecting to get a PATCH request to update CoreDB resource
            let (request, send) = handle
                .next_request()
                .await
                .expect("Kube API called to PATCH CoreDB");
            assert_eq!(request.method(), http::Method::PATCH);
            assert_eq!(
                request.uri().to_string(),
                format!(
                    "/apis/coredb.io/v1alpha1/namespaces/testns/coredbs/{}/status?&force=true&fieldManager=cntrlr",
                    coredb.name_any()
                )
            );
            let req_body = request.into_body().collect_bytes().await.unwrap();
            let json: serde_json::Value =
                serde_json::from_slice(&req_body).expect("patch_status object is json");
            let status_json = json.get("status").expect("status object").clone();
            let status: CoreDBStatus =
                serde_json::from_value(status_json).expect("contains valid status");
            assert!(
                status.running,
                "CoreDB::test says the status isn't running, but it was expected to be running."
            );

            let response = serde_json::to_vec(&coredb.with_status(status)).unwrap();
            // pass through coredb "patch accepted"
            send.send_response(Response::builder().body(Body::from(response)).unwrap());
        })
    }
}

impl Context {
    // Create a test context with a mocked kube client, unregistered metrics and default diagnostics
    pub fn test() -> (Arc<Self>, ApiServerVerifier, Registry) {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let mock_client = Client::new(mock_service, "default");
        let registry = Registry::default();
        (
            Arc::new(Self {
                client: mock_client,
                metrics: Metrics::default().register(&registry).unwrap(),
                diagnostics: Arc::default(),
            }),
            ApiServerVerifier(handle),
            registry,
        )
    }
}
