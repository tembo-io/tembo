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
    use controller::{
        apis::coredb_types::CoreDB,
        defaults::{default_resources, default_storage},
        is_pod_ready,
    };
    use k8s_openapi::{
        api::core::v1::{
            Container, Namespace, PersistentVolumeClaim, Pod, PodSpec, ResourceRequirements, Secret,
        },
        apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
        apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::ObjectMeta},
    };
    use kube::{
        api::{AttachParams, Patch, PatchParams, PostParams},
        runtime::wait::{await_condition, conditions, Condition},
        Api, Client, Config,
    };
    use rand::Rng;
    use std::{str, thread, time::Duration};
    use tokio::io::AsyncReadExt;

    const API_VERSION: &str = "coredb.io/v1alpha1";
    // Timeout settings while waiting for an event
    const TIMEOUT_SECONDS_START_POD: u64 = 120;
    const TIMEOUT_SECONDS_POD_READY: u64 = 30;
    const TIMEOUT_SECONDS_SECRET_PRESENT: u64 = 30;
    const TIMEOUT_SECONDS_NS_DELETED: u64 = 30;
    const TIMEOUT_SECONDS_COREDB_DELETED: u64 = 45;

    async fn create_test_buddy(pods_api: Api<Pod>, name: String) -> String {
        // Launch a pod we can connect to if we want to
        // run commands inside the cluster.
        let test_pod_name = format!("test-buddy-{}", name);
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some(test_pod_name.clone()),
                ..ObjectMeta::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    command: Some(vec!["sleep".to_string()]),
                    args: Some(vec!["360".to_string()]),
                    name: "test-connection".to_string(),
                    image: Some("curlimages/curl:latest".to_string()),
                    ..Container::default()
                }],
                restart_policy: Some("Never".to_string()),
                ..PodSpec::default()
            }),
            ..Pod::default()
        };

        let _pod = pods_api.create(&PostParams::default(), &pod).await.unwrap();

        test_pod_name
    }

    async fn run_command_in_container(pods_api: Api<Pod>, pod_name: String, command: Vec<String>) -> String {
        let attach_params = AttachParams {
            container: None,
            tty: false,
            stdin: true,
            stdout: true,
            stderr: true,
            max_stdin_buf_size: Some(1024),
            max_stdout_buf_size: Some(1024),
            max_stderr_buf_size: Some(1024),
        };

        let mut attached_process = pods_api
            .exec(pod_name.as_str(), &command, &attach_params)
            .await
            .unwrap();
        let mut stdout_reader = attached_process.stdout().unwrap();
        let mut result_stdout = String::new();
        stdout_reader.read_to_string(&mut result_stdout).await.unwrap();

        result_stdout
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

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
        let test_pod_name = create_test_buddy(pods.clone(), name.to_string()).await;

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
                "serviceAccountTemplate": {
                    "metadata": {
                        "annotations": {
                            "eks.amazonaws.com/role-arn": "arn:aws:iam::012345678901:role/cdb-test-iam"
                        }
                    }
                },
                "extensions": [
                    {
                        "name": "postgis",
                        "description": "PostGIS extension",
                        "locations": [{
                            "enabled": true,
                            "version": "1.1.1",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }]
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
        let _ = tokio::time::timeout(Duration::from_secs(TIMEOUT_SECONDS_SECRET_PRESENT), establish)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Did not find the secret {} present after waiting {} seconds",
                    secret_name, TIMEOUT_SECONDS_SECRET_PRESENT
                )
            });
        println!("Found secret: {}", secret_name);

        // Wait for Pod to be created
        let pod_name = format!("{}-0", name);

        println!("Waiting for pod to be running: {}", pod_name);
        let _check_for_pod = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_START_POD),
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Did not find the pod {} to be running after waiting {} seconds",
                pod_name, TIMEOUT_SECONDS_START_POD
            )
        });
        println!("Waiting for pod to be ready: {}", pod_name);
        let _check_for_pod_ready = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_POD_READY),
            await_condition(pods.clone(), &pod_name, is_pod_ready()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Did not find the pod {} to be ready after waiting {} seconds",
                pod_name, TIMEOUT_SECONDS_POD_READY
            )
        });
        println!("Found pod ready: {}", pod_name);

        // Assert default storage values are applied to PVC
        let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
        let default_storage: Quantity = default_storage();
        let pvc = pvc_api.get(&format!("data-{}", pod_name)).await.unwrap();
        let storage = pvc.spec.unwrap().resources.unwrap().requests.unwrap();
        let s = storage.get("storage").unwrap().to_owned();
        assert_eq!(default_storage, s);

        // Assert default resource values are applied to postgres container
        let default_resources: ResourceRequirements = default_resources();
        let pg_pod = pods.get(&pod_name).await.unwrap();
        let resources = pg_pod.spec.unwrap().containers[0].clone().resources;
        assert_eq!(default_resources, resources.unwrap());

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
        thread::sleep(Duration::from_millis(10000));

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

        // Assert role 'postgres_exporter' was created
        let result = coredb_resource
            .psql(
                "SELECT rolname FROM pg_roles;".to_string(),
                "postgres".to_string(),
                client.clone(),
            )
            .await
            .unwrap();

        assert!(
            result.stdout.clone().unwrap().contains("postgres_exporter"),
            "results must contain postgres_exporter: {}",
            result.stdout.clone().unwrap()
        );

        // Assert we can curl the metrics from the service
        let metrics_service_name = format!("{}-metrics", name);
        let command = vec![
            String::from("curl"),
            format!("http://{metrics_service_name}/metrics"),
        ];
        let result_stdout = run_command_in_container(pods.clone(), test_pod_name.clone(), command).await;
        assert!(result_stdout.contains("pg_up 1"));
        println!("Found metrics when curling the metrics service");

        // Assert we can drop an extension after its been created
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "serviceAccountTemplate": {
                    "metadata": {
                        "annotations": {
                            "eks.amazonaws.com/role-arn": "arn:aws:iam::012345678901:role/cdb-test-iam"
                        }
                    }
                },
                "extensions": [
                    {
                        "name": "postgis",
                        "description": "PostGIS extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.1.1",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }]
            }
        });

        // Apply crd with extension disabled
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // give it time to drop
        thread::sleep(Duration::from_millis(5000));

        // Assert extension no longer created
        let result = coredb_resource
            .psql(
                "select extname from pg_catalog.pg_extension;".to_string(),
                "postgres".to_string(),
                client.clone(),
            )
            .await
            .unwrap();

        // assert does not contain postgis
        assert!(
            !result.stdout.clone().unwrap().contains("postgis"),
            "results should not contain postgis: {}",
            result.stdout.clone().unwrap()
        );

        // assert extensions made it into the status
        let spec = coredbs.get(name).await.unwrap();
        let status = spec.status.unwrap();
        let extensions = status.extensions;
        assert!(extensions.clone().expect("expected extensions").len() > 0);
        assert!(extensions.expect("expected extensions")[0].description.len() > 0);

        // Change size of a PVC
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "pkglibdirStorage": "2Gi",
                "sharedirStorage" : "2Gi",
                "serviceAccountTemplate": {
                    "metadata": {
                        "annotations": {
                            "eks.amazonaws.com/role-arn": "arn:aws:iam::012345678901:role/cdb-test-iam"
                        }
                    }
                }
            }
        });
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _ = coredbs.patch(name, &params, &patch).await.unwrap();
        thread::sleep(Duration::from_millis(10000));
        let pvc = pvc_api.get(&format!("pkglibdir-{}", pod_name)).await.unwrap();
        // checking that the request is set, but its not the status
        // https://github.com/rancher/local-path-provisioner/issues/323
        let storage = pvc.spec.unwrap().resources.unwrap().requests.unwrap();
        let s = storage.get("storage").unwrap().to_owned();
        assert_eq!(Quantity("2Gi".to_owned()), s);
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_delete_namespace() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let mut rng = rand::thread_rng();
        let name = &format!("test-coredb-{}", rng.gen_range(0..100000));
        let namespace = name;

        // Create namespace
        let ns_api: Api<Namespace> = Api::all(client.clone());
        let params = PatchParams::apply("coredb-integration-test").force();
        let ns = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Namespace",
            "metadata": {
                "name": format!("{}", namespace),
            }
        });
        ns_api
            .patch(namespace, &params, &Patch::Apply(&ns))
            .await
            .unwrap();

        // Create coredb
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": "CoreDB",
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": 1,
                "serviceAccountTemplate": {
                    "metadata": {
                        "annotations": {
                            "eks.amazonaws.com/role-arn": "arn:aws:iam::012345678901:role/cdb-test-iam"
                        }
                    }
                },
                "extensions": [
                    {
                        "name": "postgis",
                        "description": "PostGIS extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.1.1",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }]
            }
        });
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        coredbs.patch(name, &params, &patch).await.unwrap();

        // Assert coredb is running
        let pod_name = format!("{}-0", name);
        let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
        println!("Waiting for pod to be running: {}", pod_name);
        let _check_for_pod = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_START_POD),
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Did not find the pod {} to be running after waiting {} seconds",
                pod_name, TIMEOUT_SECONDS_START_POD
            )
        });
        println!("Waiting for pod to be ready: {}", pod_name);
        let _check_for_pod_ready = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_POD_READY),
            await_condition(pods.clone(), &pod_name, is_pod_ready()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Did not find the pod {} to be ready after waiting {} seconds",
                pod_name, TIMEOUT_SECONDS_POD_READY
            )
        });
        println!("Found pod ready: {}", pod_name);

        // Delete namespace
        ns_api.delete(namespace, &Default::default()).await.unwrap();

        // Assert coredb has been deleted
        // TODO(ianstanton) This doesn't assert the object is gone for good. Tried implementing something
        //  similar to the loop used in namespace delete assertion, but received a comparison error.
        println!("Waiting for CoreDB to be deleted: {}", &name);
        let _assert_coredb_deleted = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_COREDB_DELETED),
            await_condition(coredbs.clone(), name, conditions::is_deleted("")),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "CoreDB {} was not deleted after waiting {} seconds",
                name, TIMEOUT_SECONDS_COREDB_DELETED
            )
        });

        // Assert namespace has been deleted
        println!("Waiting for namespace to be deleted: {}", &namespace);
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECONDS_NS_DELETED), async move {
            loop {
                let get_ns = ns_api.get_opt(namespace).await.unwrap();
                if get_ns.is_none() {
                    break;
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Namespace {} was not deleted after waiting {} seconds",
                namespace, TIMEOUT_SECONDS_NS_DELETED
            )
        });
    }

    #[tokio::test]
    #[ignore]
    async fn test_stop_instance() {
        // Initialize the Kubernetes client
        let client = kube_client().await;

        // Configurations
        let mut rng = rand::thread_rng();
        let name = &format!("test-stop-coredb-{}", rng.gen_range(0..100000));
        let namespace = "default";
        let kind = "CoreDB";

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
        // let test_pod_name = create_test_buddy(pods.clone(), name.to_string()).await;

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
                "stop": false
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
        let _ = tokio::time::timeout(Duration::from_secs(TIMEOUT_SECONDS_SECRET_PRESENT), establish)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Did not find the secret {} present after waiting {} seconds",
                    secret_name, TIMEOUT_SECONDS_SECRET_PRESENT
                )
            });
        println!("Found secret: {}", secret_name);

        // Wait for Pod to be created
        let pod_name = format!("{}-0", name);

        println!("Waiting for pod to be running: {}", pod_name);
        let _check_for_pod = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_START_POD),
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Did not find the pod {} to be running after waiting {} seconds",
                pod_name, TIMEOUT_SECONDS_START_POD
            )
        });
        println!("Waiting for pod to be ready: {}", pod_name);
        let _check_for_pod_ready = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_POD_READY),
            await_condition(pods.clone(), &pod_name, is_pod_ready()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Did not find the pod {} to be ready after waiting {} seconds",
                pod_name, TIMEOUT_SECONDS_POD_READY
            )
        });
        println!("Found pod ready: {}", pod_name);

        // Assert default resource values are applied to postgres container
        let default_resources: ResourceRequirements = default_resources();
        let pg_pod = pods.get(&pod_name).await.unwrap();
        let resources = pg_pod.spec.unwrap().containers[0].clone().resources;
        assert_eq!(default_resources, resources.unwrap());

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

        // Create a table
        let result = coredb_resource
            .psql(
                "
                CREATE TABLE stop_test (
                   id serial PRIMARY KEY,
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

        // Assert table exists
        let result = coredb_resource
            .psql("\\dt".to_string(), "postgres".to_string(), client.clone())
            .await
            .unwrap();
        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("stop_test"));

        thread::sleep(Duration::from_millis(5000));

        // stop the instance
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "stop": true,
            }
        });

        // Apply crd with stop flag enabled
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // give it time to stop
        thread::sleep(Duration::from_millis(5000));

        // pod must not be ready
        let res = coredb_resource.primary_pod(client.clone()).await;
        assert!(res.is_err());

        // start again
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "stop": false,
            }
        });
        // Apply with stop flag disabled
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // give it time to start
        println!("Waiting for pod to be running: {}", pod_name);
        let check_for_pod = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_START_POD),
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        );
        assert!(check_for_pod.await.is_ok());
        // assert table still exist
        let result = coredb_resource
            .psql("\\dt".to_string(), "postgres".to_string(), client.clone())
            .await
            .unwrap();
        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("stop_test"));
    }

    async fn kube_client() -> Client {
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
        let namespace = namespaces.get(selected_namespace).await.unwrap();
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
            Duration::from_secs(2),
            await_condition(
                custom_resource_definitions,
                "coredbs.coredb.io",
                conditions::is_crd_established(),
            ),
        )
        .await
        .expect("Custom Resource Definition for CoreDB was not found.");

        client
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
