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
    use k8s_openapi::{
        api::{core::v1::Namespace, core::v1::PersistentVolumeClaim, core::v1::Pod},
        apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
    };

    use kube::{
        runtime::wait::{await_condition, conditions},
        Api, Client, Config,
    };
    use pgmq::{Message, PGMQueueExt};

    use conductor::get_coredb_error_without_status;
    use conductor::types::{self, StateToControlPlane};
    use controller::extensions::types::{Extension, ExtensionInstallLocation};
    use controller::{
        apis::coredb_types::{CoreDB, CoreDBSpec},
        is_pod_ready,
        postgres_exporter::{PostgresMetrics, QueryConfig},
        State,
    };
    use rand::Rng;
    use std::collections::BTreeMap;
    use std::{thread, time, time::Duration};

    // Timeout settings while waiting for an event
    const TIMEOUT_SECONDS_START_POD: u64 = 600;
    const TIMEOUT_SECONDS_POD_READY: u64 = 600;

    // helper to poll for messages from data plane queue with retries
    async fn get_dataplane_message(
        retries: u64,
        retry_delay_seconds: u64,
        queue: &PGMQueueExt,
    ) -> Message<StateToControlPlane> {
        // wait for conductor to send message to data_plane_events queue
        let mut attempt = 0;

        let msg: Option<Message<StateToControlPlane>> = loop {
            attempt += 1;
            if attempt > retries {
                panic!(
                    "No message found in data plane queue after - {} - retries",
                    retries
                );
            } else {
                // read message from data_plane_events queue
                let msg = queue
                    .read::<StateToControlPlane>("myqueue_data_plane", 30_i32)
                    .await
                    .expect("database error");
                if msg.is_some() {
                    break msg;
                } else {
                    thread::sleep(time::Duration::from_secs(retry_delay_seconds));
                }
            };
        };
        msg.expect("no message found")
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_basic_create() {
        let queue = PGMQueueExt::new("postgres://postgres:postgres@0.0.0.0:5431".to_owned(), 1)
            .await
            .unwrap();
        queue.init().await.expect("failed creating extension");

        let myqueue = "myqueue_control_plane".to_owned();
        let _ = queue.create(&myqueue).await;

        // Configurations
        let mut rng = rand::thread_rng();
        let org_name = "coredb-test-org-1234".to_owned();
        // Use the max allowed org name length
        assert_eq!(org_name.len(), 20);
        let dbname = format!("test-coredb-{}", rng.gen_range(10000000..99999999));
        // Use the max allowed instance name length
        assert_eq!(dbname.len(), 20);
        let namespace = format!("org-{}-inst-{}", org_name, dbname);

        let limits: BTreeMap<String, String> = BTreeMap::from([
            ("cpu".to_owned(), "1".to_string()),
            ("memory".to_owned(), "1Gi".to_string()),
        ]);

        let custom_metrics = serde_json::json!({
          "pg_postmaster": {
            "query": "SELECT pg_postmaster_start_time as start_time_seconds from pg_postmaster_start_time()",
            "master": true,
            "metrics": [
              {
                "start_time_seconds": {
                  "usage": "GAUGE",
                  "description": "Time at which postmaster started"
                }
              }
            ]
          },
          "extensions": {
            "query": "select count(*) as num_ext from pg_available_extensions",
            "master": true,
            "metrics": [
              {
                "num_ext": {
                  "usage": "GAUGE",
                  "description": "Num extensions"
                }
              }
            ]
          }
        });
        let query_config: QueryConfig =
            serde_json::from_value(custom_metrics).expect("failed to deserialize");

        // conductor receives a CRUDevent from control plane
        let install_location = ExtensionInstallLocation {
            enabled: true,
            version: Some("1.3.0".to_owned()),
            database: "postgres".to_owned(),
            ..ExtensionInstallLocation::default()
        };
        let install_location = install_location.clone();
        let spec_js = serde_json::json!({
            "extensions": Some(vec![Extension {
                name: "aggs_for_vecs".to_owned(),
                description: Some("aggs_for_vecs extension".to_owned()),
                locations: vec![install_location],
            }]),
            "storage": Some("1Gi".to_owned()),
            "replicas": Some(1),
            "resources":
                serde_json::json!({
                    "limits": limits,
                }),
            "metrics": Some(PostgresMetrics{
                queries: Some(query_config),
                enabled: true,
                image: "default-image-value".to_string()
            })
        });
        let mut spec: CoreDBSpec = serde_json::from_value(spec_js).unwrap();

        let msg = types::CRUDevent {
            namespace: namespace.clone(),
            backups_read_path: None,
            backups_write_path: None,
            data_plane_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            org_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            inst_id: "inst_02s4UKVbRy34SAYVSwZq2H".to_owned(),
            event_type: types::Event::Create,
            spec: Some(spec.clone()),
        };

        // println!("Message: {:?}", msg);

        let msg_id = queue.send(&myqueue, &msg).await;
        println!("Create msg_id: {msg_id:?}");

        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        let timeout_seconds_start_pod = 300;

        let pod_name = format!("{namespace}-1");

        let _check_for_pod = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_seconds_start_pod),
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Did not find the pod {} to be running after waiting {} seconds",
                pod_name, timeout_seconds_start_pod
            )
        });

        // wait for conductor to send message to data_plane_events queue
        let retries = 120;
        let retry_delay = 2;
        let msg = get_dataplane_message(retries, retry_delay, &queue).await;

        queue
            .archive("myqueue_data_plane", msg.msg_id)
            .await
            .expect("error deleting message");

        let passed_spec = msg.message.spec.expect("No spec found in message");

        // assert that the message returned by Conductor includes the new metrics values in the spec
        // println!("spec: {:?}", passed_spec);
        assert!(passed_spec
            .metrics
            .expect("no metrics in data-plane-event message")
            .queries
            .expect("queries missing")
            .queries
            .contains_key("pg_postmaster"));

        assert!(
            !passed_spec.extensions.is_empty(),
            "Extension object missing from spec"
        );
        let extensions = passed_spec.extensions.clone();
        assert!(
            !extensions.is_empty(),
            "Expected at least one extension: {:?}",
            extensions
        );

        let coredb_api: Api<CoreDB> = Api::namespaced(client.clone(), &namespace.clone());
        let coredb_resource = coredb_api.get(&namespace.clone()).await.unwrap();

        // Wait for CNPG pod to be running and ready
        let pod_name = format!("{}-1", &namespace.clone());
        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // ADD AN EXTENSION - ASSERT IT MAKES IT TO STATUS.EXTENSIONS
        // conductor receives a CRUDevent from control plane
        // take note of number of extensions at this point in time
        // let mut extensions_add = extensions.clone();
        let _install_location = ExtensionInstallLocation::default();
        let install_location = ExtensionInstallLocation {
            enabled: true,
            version: Some("0.1.4".to_owned()),
            database: "postgres".to_owned(),
            ..ExtensionInstallLocation::default()
        };
        let install_location = install_location.clone();
        spec.extensions.push(Extension {
            name: "pg_jsonschema".to_owned(),
            description: Some("fake description".to_string()),
            locations: vec![install_location],
        });
        let num_expected_extensions = spec.extensions.len();
        // Get the current CoreDB spec
        let current_coredb = coredb_resource.clone();
        // println!("Updated spec: {:?}", spec.clone());
        let msg = types::CRUDevent {
            namespace: namespace.clone(),
            backups_read_path: None,
            backups_write_path: None,
            data_plane_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            org_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            inst_id: "inst_02s4UKVbRy34SAYVSwZq2H".to_owned(),
            event_type: types::Event::Update,
            spec: Some(spec.clone()),
        };
        let msg_id = queue.send(&myqueue, &msg).await;
        println!("Update msg_id: {msg_id:?}");

        // read message from data_plane_events queue
        let mut extensions: Vec<Extension> = vec![];
        while num_expected_extensions != extensions.len() {
            let msg = get_dataplane_message(retries, retry_delay, &queue).await;
            // println!("Update msg: {:?}", msg);
            queue
                .archive("myqueue_data_plane", msg.msg_id)
                .await
                .expect("error deleting message");

            extensions = msg
                .message
                .spec
                .expect("No spec found in message")
                .extensions;
        }
        // we added an extension, so it should be +1 now
        assert_eq!(num_expected_extensions, extensions.len());

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // Get the last time the pod was started
        // using SELECT pg_postmaster_start_time();
        let start_time = match coredb_resource
            .psql(
                "SELECT pg_postmaster_start_time();".to_string(),
                "postgres".to_string(),
                context.clone(),
            )
            .await
        {
            Ok(res) => res,
            Err(e) => panic!("Failed to execute psql for start_time: {:?}", e),
        };

        let stdout = match start_time.stdout {
            Some(output) => output,
            None => panic!("stdout for start_time is None"),
        };

        println!("start_time: {:?}", stdout);

        // Since we have updated lets check the status of the CoreDB from current_coredb
        let update_coredb = coredb_api.get(&namespace.clone()).await.unwrap();
        let old_backup_spec = current_coredb.spec.backup.clone();
        let new_backup_spec = update_coredb.spec.backup.clone();

        // assert that the backup.schedule for old_backup_spec are equal to new_backup_spec
        assert_eq!(old_backup_spec.schedule, new_backup_spec.schedule);

        // assert that the destination paths for old_backup_spec are equal to new_backup_spec
        assert_eq!(
            old_backup_spec.destinationPath,
            new_backup_spec.destinationPath
        );

        // Lets now test sending an Event::Restart to the queue and see if the
        // pod restarts correctly.

        let msg = types::CRUDevent {
            namespace: namespace.clone(),
            backups_read_path: None,
            backups_write_path: None,
            data_plane_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            org_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            inst_id: "inst_02s4UKVbRy34SAYVSwZq2H".to_owned(),
            event_type: types::Event::Restart,
            spec: Some(spec.clone()),
        };
        let msg_id = queue.send(&myqueue, &msg).await;
        println!("Restart msg_id: {:?}", msg_id);

        let mut is_ready = false;
        let mut current_iteration = 0;
        while !is_ready {
            if current_iteration > 30 {
                panic!("CNPG pod did not restart after about 300 seconds");
            }
            thread::sleep(time::Duration::from_secs(10));
            let current_coredb =
                get_coredb_error_without_status(client.clone(), &namespace.clone())
                    .await
                    .unwrap();
            if let Some(status) = current_coredb.status {
                if status.running {
                    is_ready = true;
                }
            }
            current_iteration += 1;
        }

        pod_ready_and_running(pods.clone(), pod_name).await;

        // Get the last time the pod was started
        // using SELECT pg_postmaster_start_time();
        // and compare to the previous time
        let restart_time = coredb_resource
            .psql(
                "SELECT pg_postmaster_start_time();".to_string(),
                "postgres".to_string(),
                context.clone(),
            )
            .await
            .unwrap();

        println!("restart_time: {:?}", restart_time.stdout.clone().unwrap());
        // assert that restart_time is greater than start_time
        // TODO: https://linear.app/tembo/issue/TEM-1296/status-on-an-instance-shows-ok-even-during-restart
        // assert!(restart_time.stdout.clone().unwrap() > start_time.stdout.clone().unwrap());

        // delete the instance
        let msg = types::CRUDevent {
            namespace: namespace.clone(),
            backups_write_path: None,
            backups_read_path: None,
            data_plane_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            org_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            inst_id: "inst_02s4UKVbRy34SAYVSwZq2H".to_owned(),
            event_type: types::Event::Delete,
            spec: None,
        };
        // println!("DELETE msg: {:?}", msg);
        let msg_id = queue.send(&myqueue, &msg).await;
        println!("Delete msg_id: {msg_id:?}");

        // Wait for CoreDB to be deleted
        let cdb_delete =
            wait_until_resource_deleted(client.clone(), &namespace, &namespace, "CoreDB").await;
        assert!(cdb_delete);

        // Wait for pvc to be deleted
        let pvc_delete = wait_until_resource_deleted(
            client.clone(),
            &namespace,
            &namespace,
            "PersistentVolumeClaim",
        )
        .await;
        assert!(pvc_delete);

        // Wait for namespace to be deleted
        let ns_delete =
            wait_until_resource_deleted(client.clone(), &namespace, &namespace, "Namespace").await;
        assert!(ns_delete);

        // call aws api and verify CF stack was deleted
        use aws_sdk_cloudformation::config::Region;
        use conductor::aws::cloudformation::AWSConfigState;
        let aws_region = "us-east-1".to_owned();
        let region = Region::new(aws_region);
        let aws_config_state = AWSConfigState::new(region.clone()).await;
        let stack_name = format!("org-{}-inst-{}-cf", org_name, dbname);

        // Check to see if the cloudformation stack exists
        let cf_stack_deleted = check_cf_stack_deletion(&aws_config_state, &stack_name).await;
        assert!(cf_stack_deleted, "CF stack was deleted");
    }

    async fn kube_client() -> kube::Client {
        // Get the name of the currently selected namespace
        let kube_config = Config::infer()
            .await
            .expect("Please configure your Kubernetes context.");

        // Initialize the Kubernetes client
        let client =
            Client::try_from(kube_config.clone()).expect("Failed to initialize Kubernetes client");

        // Next, check that the currently selected namespace is labeled
        // to allow the running of tests.

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
        .expect("Custom Resource Definition for CoreDB was not found, do you need to install that before running the tests?");
        client
    }

    async fn pod_ready_and_running(pods: Api<Pod>, pod_name: String) {
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
    }

    async fn wait_until_resource_deleted(
        client: Client,
        namespace: &str,
        name: &str,
        resource_type: &str,
    ) -> bool {
        for _ in 0..60 {
            let result: Result<(), kube::Error> = match resource_type {
                "Namespace" => {
                    let api: Api<Namespace> = Api::all(client.clone());
                    api.get(name).await.map(|_| ())
                }
                "PersistentVolumeClaim" => {
                    let api: Api<PersistentVolumeClaim> =
                        Api::namespaced(client.clone(), namespace);
                    api.get(name).await.map(|_| ())
                }
                "CoreDB" => {
                    let api: Api<CoreDB> = Api::namespaced(client.clone(), namespace);
                    api.get(name).await.map(|_| ())
                }
                _ => {
                    println!("Unsupported resource type");
                    return false;
                }
            };

            if result.is_err() {
                return true; // Resource is deleted
            }

            println!(
                "Waiting for resource {} of type {} to be deleted...",
                name, resource_type
            );
            thread::sleep(time::Duration::from_secs(5));
        }
        false
    }

    use conductor::aws::cloudformation::AWSConfigState;
    async fn check_cf_stack_deletion(acs: &AWSConfigState, stack_name: &str) -> bool {
        let max_duration = Duration::from_secs(5 * 60); // 5 minutes
        let check_interval = Duration::from_secs(30); // Check every 30 seconds

        let start_time = tokio::time::Instant::now();
        while tokio::time::Instant::now() - start_time < max_duration {
            let exists = acs.does_stack_exist(stack_name).await;
            println!("Checking if CF stack {} exists: {}", stack_name, exists);

            if !exists {
                println!(
                    "CF stack {} does not exist, we assume it's deleted",
                    stack_name
                );
                return true;
            } else {
                match acs.delete_cloudformation_stack(stack_name).await {
                    Ok(_) => {
                        println!("CF stack {} was deleted", stack_name);
                    }
                    Err(e) => {
                        panic!("Failed to delete CloudFormation stack: {:?}", e);
                    }
                }
            }

            tokio::time::sleep(check_interval).await;
        }

        println!(
            "CF stack {} was not deleted within the expected time.",
            stack_name
        );
        false
    }
}
