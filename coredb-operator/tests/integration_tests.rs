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

    use controller::{is_pod_ready, CoreDB};
    use k8s_openapi::{
        api::core::v1::{Namespace, Pod, Secret},
        apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
    };
    use kube::{
        api::{Patch, PatchParams},
        runtime::wait::{await_condition, conditions, Condition},
        Api, Client, Config,
    };
    use rand::Rng;
    use std::{str, thread, time::Duration};

    const API_VERSION: &str = "coredb.io/v1alpha1";

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
        let timeout_seconds_secret_present = 30;

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
                "replicas": replicas,
                "enabledExtensions": ["postgis"]
            }
        });
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for secret to be created
        let secret_api: Api<Secret> = Api::namespaced(client.clone(), namespace);
        let secret_name = format!("{}-connection", name);
        println!("Waiting for secret to be created: {}", secret_name);
        let establish = await_condition(secret_api.clone(), &secret_name, wait_for_secret());
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_seconds_secret_present),
            establish,
        )
        .await
        .expect(&format!(
            "Did not find the secret {} present after waiting {} seconds",
            secret_name, timeout_seconds_secret_present
        ));
        println!("Found secret: {}", secret_name);

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

        // Assert no tables found
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

        // Create table 'customers'
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

        // Assert table 'customers' exists
        let result = coredb_resource
            .psql("\\dt".to_string(), "postgres".to_string(), client.clone())
            .await
            .unwrap();
        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("customers"));

        // TODO(ianstanton) we need to properly wait for 'postgis' extension to be created
        thread::sleep(Duration::from_millis(500));

        // Assert extension 'postgis' was created
        let result = coredb_resource
            .psql(
                "select extname from pg_catalog.pg_extension;".to_string(),
                "postgres".to_string(),
                client.clone(),
            )
            .await
            .unwrap();

        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("postgis"));

        // TODO(ianstanton) Tear down resources when finished.
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

    fn wait_for_secret() -> impl Condition<Secret> {
        |obj: Option<&Secret>| {
            if let Some(secret) = &obj {
                if let Some(t) = &secret.type_ {
                    return t == "Opaque";
                }
            }
            false
        }
    }
}
