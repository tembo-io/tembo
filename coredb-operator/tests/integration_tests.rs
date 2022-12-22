// Include the #[ignore] macro on slow tests.
// That way, 'cargo test' does not run them by default.
// To run just these tests, use 'cargo test -- --ignored'
// To run all tests, use 'cargo test -- --include-ignored'
//
// https://doc.rust-lang.org/book/ch11-02-running-tests.html
//
// These tests assume there is already kubernetes running and you have a context configured.
// It also assumes that the CRD(s) and operator are already installed for this cluster.
// In this way, it can be used as a conformance test on a target, separate from installation.

#[cfg(test)]
mod test {

    use controller::CoreDB;
    use k8s_openapi::{
        api::core::v1::{Namespace, Pod},
        apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
    };
    use kube::{
        api::{Patch, PatchParams},
        runtime::wait::{await_condition, conditions},
        Api, Client, Config,
    };
    use rand::Rng;

    const API_VERSION: &str = "kube.rs/v1";

    #[tokio::test]
    #[ignore]
    async fn functional_test_basic_create() {
        // Initialize the Kubernetes client
        let client = kube_client().await;

        // Configurations
        let mut rng = rand::thread_rng();
        let name = &format!("test-coredb-{}", rng.gen_range(0..100000));
        let namespace = "default";
        let kind = "CoreDB";
        let replicas = 1;

        // Timeout settings while waiting for an event
        let timeout_seconds = 60;

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas
            }
        });
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await;

        // Wait for Pod to be created

        let pod_name = format!("{}-0", name);
        println!("Waiting for pod to be running: {}", pod_name);
        let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
        if let Err(_) = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_seconds),
            await_condition(pods, &pod_name, conditions::is_pod_running()),
        )
        .await
        {
            panic!(
                "\n\nERROR: Did not find the pod {} to be running after waiting for {} seconds\n\n",
                pod_name, timeout_seconds
            )
        }
    }

    async fn kube_client() -> kube::Client {
        // Initialize the Kubernetes client
        let client_future = Client::try_default();
        let client = match client_future.await {
            Ok(wrapped_client) => wrapped_client,
            Err(_error) => panic!("Please configure your Kubernetes Context"),
        };
        // Get the name of the currently selected namespace
        let selected_namespace = Config::infer().await.unwrap().default_namespace;

        // Next, check that the currently selected namespace is labeled
        // to allow the running of tests.

        // List the namespaces with the specified labels
        let namespaces: Api<Namespace> = Api::all(client.clone());
        let namespace = namespaces.get(&selected_namespace).await.unwrap();
        let labels = namespace.metadata.labels.unwrap();
        assert!(
            labels.contains_key("safe-to-run-coredb-tests"),
            "expected to find label 'safe-to-run-coredb-tests'"
        );
        assert_eq!(
            labels["safe-to-run-coredb-tests"], "true",
            "expected to find label 'safe-to-run-coredb-tests' with value 'true'"
        );

        // Check that the CRD is installed
        let crds: Api<CustomResourceDefinition> = Api::all(client.clone());
        if let Err(_) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            await_condition(crds, "coredbs.kube.rs", conditions::is_crd_established()),
        )
        .await
        {
            panic!("\n\nERROR: Did not find the CRD to be installed.\n\n")
        }

        return client;
    }
}
