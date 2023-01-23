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
        runtime::wait::{await_condition, conditions, Condition},
        Api, Client, Config,
    };
    use rand::Rng;
    use std::str;

    const API_VERSION: &str = "coredb.io/v1";

    fn is_pod_ready() -> impl Condition<Pod> + 'static {
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
        let timeout_seconds_start_pod = 60;
        let timeout_seconds_pod_ready = 30;

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
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for Pod to be created

        let pod_name = format!("{}-0", name);
        let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
        println!("Waiting for pod to be running: {}", pod_name);
        let _check_for_pod = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_seconds_start_pod),
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        )
        .await
        .expect(&format!(
            "Did not find the pod {} to be running after waiting {} seconds",
            pod_name, timeout_seconds_start_pod
        ));
        println!("Waiting for pod to be ready: {}", pod_name);
        let _check_for_pod_ready = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_seconds_pod_ready),
            await_condition(pods.clone(), &pod_name, is_pod_ready()),
        )
        .await
        .expect(&format!(
            "Did not find the pod {} to be ready after waiting {} seconds",
            pod_name, timeout_seconds_pod_ready
        ));
        println!("Found pod ready: {}", pod_name);
        let result = coredb_resource
            .psql("\\dt".to_string(), "postgres".to_string(), client.clone())
            .await
            .unwrap();
        println!("{}", result.stderr.clone().unwrap());
        assert!(result
            .stderr
            .clone()
            .unwrap()
            .contains("Did not find any relations."));
        let result = coredb_resource
            .psql(
                "
                CREATE TABLE customers (
                   id serial PRIMARY KEY,
                   name VARCHAR(50) NOT NULL,
                   email VARCHAR(50) NOT NULL UNIQUE,
                   created_at TIMESTAMP DEFAULT NOW()
                );
                "
                .to_string(),
                "postgres".to_string(),
                client.clone(),
            )
            .await
            .unwrap();
        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("CREATE TABLE"));
        let result = coredb_resource
            .psql("\\dt".to_string(), "postgres".to_string(), client.clone())
            .await
            .unwrap();
        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("customers"));
    }

    async fn kube_client() -> kube::Client {
        // Get the name of the currently selected namespace
        let kube_config = Config::infer()
            .await
            .expect("Please configure your Kubernetes context.");
        let selected_namespace = &kube_config.default_namespace;

        // Initialize the Kubernetes client
        let client = Client::try_from(kube_config.clone()).expect("Failed to initialize Kubernetes client");

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
        let custom_resource_definitions: Api<CustomResourceDefinition> = Api::all(client.clone());

        let _check_for_crd = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            await_condition(
                custom_resource_definitions,
                "coredbs.coredb.io",
                conditions::is_crd_established(),
            ),
        )
        .await
        .expect("Custom Resource Definition for CoreDB was not found.");

        return client;
    }
}
