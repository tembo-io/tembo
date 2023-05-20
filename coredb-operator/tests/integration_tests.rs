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
    use chrono::{DateTime, SecondsFormat, Utc};
    use controller::{
        apis::coredb_types::CoreDB,
        defaults::{default_resources, default_storage},
        is_pod_ready,
    };
    use k8s_openapi::{
        api::{
            apps::v1::StatefulSet,
            batch::v1::CronJob,
            core::v1::{
                Container, Namespace, PersistentVolumeClaim, Pod, PodSpec, ResourceRequirements, Secret,
                ServiceAccount,
            },
            rbac::v1::{Role, RoleBinding},
        },
        apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
        apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::ObjectMeta},
    };
    use kube::{
        api::{AttachParams, ListParams, Patch, PatchParams, PostParams},
        runtime::wait::{await_condition, conditions, Condition},
        Api, Client, Config,
    };
    use rand::Rng;
    use std::{collections::BTreeMap, str, thread, time::Duration};
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
        let test_metric_decr = format!("coredb_integration_test_{}", rng.gen_range(0..100000));
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
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
                    }],
                "metrics": {
                    "enabled": true,
                    "queries": {
                        "test_ns": {
                            "query": "SELECT pg_postmaster_start_time as start_time_seconds from pg_postmaster_start_time()",
                            "master": true,
                            "metrics": [
                              {
                                "start_time_seconds": {
                                  "usage": "Gauge",
                                  "description": test_metric_decr
                                }
                              }
                            ]
                        },
                    }
                }
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

        // Update the coredb resource to add rbac
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "serviceAccountTemplate": {
                    "metadata": {
                        "annotations": {
                            "eks.amazonaws.com/role-arn": "arn:aws:iam::012345678901:role/cdb-test-iam"
                        }
                    }
                }
            }
        });

        // apply CRD with serviceAccountTemplate set
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // give it some time 500ms
        thread::sleep(Duration::from_millis(5000));

        // Assert that the service account exists
        let sa_api: Api<ServiceAccount> = Api::namespaced(client.clone(), namespace);
        let sa_name = format!("{}-sa", name);
        let sa = sa_api.get(&sa_name).await.unwrap();

        // Check if the service account is set correctly
        assert_eq!(sa.metadata.name.unwrap(), sa_name);

        // Check if the annotation is set correctly
        assert_eq!(
            sa.metadata.annotations.unwrap().get("eks.amazonaws.com/role-arn"),
            Some(&"arn:aws:iam::012345678901:role/cdb-test-iam".to_string())
        );

        // Assert that the role exists
        let role_api: Api<Role> = Api::namespaced(client.clone(), namespace);
        let role_name = format!("{}-role", name);
        let role = role_api.get(&role_name).await.unwrap();
        assert_eq!(role.metadata.name.unwrap(), role_name);

        // Assert that the role binding exists
        let rb_api: Api<RoleBinding> = Api::namespaced(client.clone(), namespace);
        let rb_name = format!("{}-role-binding", name);
        let role_binding = rb_api.get(&rb_name).await.unwrap();
        assert_eq!(role_binding.metadata.name.unwrap(), rb_name);

        // Get the StatefulSet
        let stateful_sets_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
        let stateful_set_name = format!("{}", name);
        let stateful_set = stateful_sets_api.get(&stateful_set_name).await.unwrap();

        //println!("stateful_set: {:#?}", stateful_set_name);

        // Assert that the StatefulSet has the correct service account
        let stateful_set_service_account_name = stateful_set
            .spec
            .as_ref()
            .unwrap()
            .template
            .spec
            .as_ref()
            .unwrap()
            .service_account_name
            .as_ref();
        assert_eq!(stateful_set_service_account_name, Some(&sa_name));

        // Restart the StatefulSet to apply updates to the running pod
        let mut stateful_set_updated = stateful_set.clone();
        stateful_set_updated
            .spec
            .as_mut()
            .unwrap()
            .template
            .metadata
            .as_mut()
            .unwrap()
            .annotations
            .get_or_insert_with(BTreeMap::new)
            .insert(
                "kubectl.kubernetes.io/restartedAt".to_string(),
                Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            );

        let params = PatchParams::default();
        let patch = Patch::Merge(&stateful_set_updated);
        let _stateful_set_patched = stateful_sets_api
            .patch(&stateful_set_name, &params, &patch)
            .await
            .unwrap();

        // Wait for the pod to restart
        let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
        let lp = ListParams::default().labels(format!("statefulset={}", stateful_set_name).as_str());

        //let restart_time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let restart_time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true).to_string();

        loop {
            let pods = pods_api.list(&lp).await.unwrap();

            let all_pods_ready_and_restarted = pods.iter().all(|pod| {
                let pod_restart_time = pod
                    .metadata
                    .annotations
                    .as_ref()
                    .and_then(|annotations| annotations.get("kubectl.kubernetes.io/restartedAt"))
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)) // Convert to DateTime<Utc>
                    .unwrap_or_else(|| Utc::now());
                let restart_time_as_datetime = DateTime::parse_from_rfc3339(&restart_time)
                    .unwrap()
                    .with_timezone(&Utc);
                pod_restart_time > restart_time_as_datetime
                    && pod
                        .status
                        .as_ref()
                        .and_then(|status| status.container_statuses.as_ref())
                        .map(|container_statuses| container_statuses.iter().all(|cs| cs.ready))
                        .unwrap_or(false)
            });

            if all_pods_ready_and_restarted {
                break;
            }

            thread::sleep(Duration::from_secs(15));
        }

        // Check the pods service account
        //let all_pods = pods_api.list(&ListParams::default()).await.unwrap();
        //println!("All pods in the namespace: {:?}", all_pods);

        let pods = pods_api.list(&lp).await.unwrap();
        let pod = match pods.iter().next() {
            Some(pod) => pod,
            None => {
                println!("Expected label: {}", format!("statefulset={}", stateful_set_name));
                panic!("No matching pods found")
            }
        };

        let pod_service_account_name = pod.spec.as_ref().unwrap().service_account_name.as_ref();
        assert_eq!(pod_service_account_name, Some(&sa_name));

        // Update the coredb resource to add backups
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "backup": {
                    "destinationPath": "s3://test-bucket/coredb/test-org/test-db",
                    "encryption": "AES256",
                    "retentionPolicy": "30",
                    "schedule": "0 0 * * *",
                }
            }
        });

        // apply CRD with serviceAccountTemplate set
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // give it some time 500ms
        thread::sleep(Duration::from_millis(5000));

        // assert that the destinationPath is set in the sts env
        let stateful_sets_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
        let stateful_set_name = format!("{}", name);
        let stateful_set = stateful_sets_api.get(&stateful_set_name).await.unwrap();

        // Extract the environment variables from the StatefulSet
        if let Some(container) = stateful_set
            .spec
            .as_ref()
            .unwrap()
            .template
            .spec
            .as_ref()
            .and_then(|s| s.containers.get(0))
        {
            if let Some(env) = container.env.as_ref() {
                let destination_path_env = env
                    .iter()
                    .find(|e| e.name == "WALG_S3_PREFIX")
                    .and_then(|e| e.value.clone());
                let walg_s3_sse_env = env
                    .iter()
                    .find(|e| e.name == "WALG_S3_SSE")
                    .and_then(|e| e.value.clone());

                assert_eq!(
                    destination_path_env,
                    Some(String::from("s3://test-bucket/coredb/test-org/test-db"))
                );
                assert_eq!(walg_s3_sse_env, Some(String::from("AES256")));
            } else {
                panic!("No environment variables found in the StatefulSet's container");
            }
        } else {
            panic!("No container found in the StatefulSet's template spec");
        }

        // Assert that the CronJob was created and the schedule is set
        let cron_jobs_api: Api<CronJob> = Api::namespaced(client.clone(), namespace);
        let cron_job_name = format!("{}-daily", name);
        let cron_job = cron_jobs_api.get(&cron_job_name).await.unwrap();

        assert_eq!(
            cron_job.spec.as_ref().unwrap().schedule,
            String::from("0 0 * * *")
        );

        let pod_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
        let cmd = vec![
            "wget".to_owned(),
            "-qO-".to_owned(),
            "http://localhost:9187/metrics".to_owned(),
        ];
        let result_stdout = run_command_in_container(pod_api.clone(), test_pod_name.clone(), cmd).await;
        assert!(result_stdout.contains(&test_metric_decr));
        println!("Found metrics when curling the metrics service");
    }

    #[tokio::test]
    #[ignore]
    async fn function_test_skip_reconciliation() {
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

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name,
                "annotations": {
                    "coredbs.coredb.io/watch": "false"
                }
            },
            "spec": {
                "replicas": replicas,
            }
        });
        let params = PatchParams::apply("coredb-integration-test-skip-reconciliation");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for the pod to be created (it shouldn't be created)
        thread::sleep(Duration::from_millis(5000));

        // Assert that the CoreDB object contains the correct annotation
        let coredb = coredbs.get(name).await.unwrap();
        let annotations = coredb.metadata.annotations.as_ref().unwrap();
        assert_eq!(
            annotations.get("coredbs.coredb.io/watch"),
            Some(&String::from("false"))
        );

        // Assert that the pod was not created
        let expected_pod_name = format!("{}-{}", name, 0);
        let pod = pods.get(&expected_pod_name).await;
        assert!(pod.is_err());
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
        thread::sleep(Duration::from_millis(8000));

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
