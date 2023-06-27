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
    use k8s_openapi::{
        api::{
            apps::v1::StatefulSet, core::v1::Namespace, core::v1::PersistentVolumeClaim,
            core::v1::Pod,
        },
        apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
    };

    use kube::{
        api::ListParams,
        runtime::wait::{await_condition, conditions},
        Api, Client, Config,
    };
    use pgmq::{Message, PGMQueueExt};

    use conductor::{
        restart_statefulset,
        types::{self, StateToControlPlane},
    };
    use controller::apis::coredb_types::{CoreDB, CoreDBSpec};
    use controller::extensions::{Extension, ExtensionInstallLocation};
    use controller::postgres_exporter::{PostgresMetrics, QueryConfig};
    use rand::Rng;
    use std::collections::BTreeMap;
    use std::{thread, time};

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
        let queue = PGMQueueExt::new("postgres://postgres:postgres@0.0.0.0:5432".to_owned(), 1)
            .await
            .unwrap();
        queue.init().await.expect("failed creating extension");

        let myqueue = "myqueue_control_plane".to_owned();
        let _ = queue.create(&myqueue).await;

        // Configurations
        let mut rng = rand::thread_rng();
        let org_name = "coredb-test-org".to_owned();
        let dbname = format!("test-coredb-{}", rng.gen_range(0..100000));
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
        let spec_js = serde_json::json!({
            "extensions": Some(vec![Extension {
                name: "aggs_for_vecs".to_owned(),
                description: Some("aggs_for_vecs extension".to_owned()),
                locations: vec![ExtensionInstallLocation {
                    enabled: true,
                    version: Some("1.3.0".to_owned()),
                    schema: "public".to_owned(),
                    database: "postgres".to_owned(),
                }],
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
        let spec: CoreDBSpec = serde_json::from_value(spec_js).unwrap();

        let msg = types::CRUDevent {
            organization_name: org_name.clone(),
            data_plane_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            event_id: format!(
                "{name}.org_02s3owPQskuGXHE8vYsGSY.CoreDB.inst_02s4UKVbRy34SAYVSwZq2H",
                name = dbname
            ),
            event_type: types::Event::Create,
            dbname: dbname.clone(),
            spec: Some(spec),
        };

        let msg_id = queue.send(&myqueue, &msg).await;
        println!("msg_id: {msg_id:?}");

        let client = kube_client().await;

        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        let timeout_seconds_start_pod = 90;

        let pod_name = format!("{namespace}-0");

        let _check_for_pod = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_seconds_start_pod),
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        )
        .await
        .unwrap_or_else(|_| panic!("Did not find the pod {pod_name} to be running after waiting {timeout_seconds_start_pod} seconds"));

        // wait for conductor to send message to data_plane_events queue
        let retries = 90;
        let retry_delay = 2;
        let msg = get_dataplane_message(retries, retry_delay, &queue).await;

        queue
            .archive("myqueue_data_plane", msg.msg_id)
            .await
            .expect("error deleting message");

        let spec = msg.message.spec.expect("No spec found in message");

        // assert that the message returned by Conductor includes the new metrics values in the spec
        assert!(spec
            .metrics
            .expect("no metrics in data-plane-event message")
            .queries
            .expect("queries missing")
            .queries
            .contains_key("pg_postmaster"));

        assert!(
            spec.extensions.len() > 0,
            "Extension object missing from spec"
        );
        let extensions = spec.extensions;
        assert!(
            !extensions.is_empty(),
            "Expected at least one extension: {:?}",
            extensions
        );

        restart_statefulset(client.clone(), &namespace, &namespace)
            .await
            .expect("failed restarting statefulset");
        thread::sleep(time::Duration::from_secs(10));
        // Verify that the statefulSet was updated with the restartedAt annotation
        let sts: Api<StatefulSet> = Api::namespaced(client.clone(), &namespace);
        let updated_statefulset = sts
            .get(&namespace)
            .await
            .expect("Failed to get StatefulSet");
        let annot = updated_statefulset
            .spec
            .expect("no spec found")
            .template
            .metadata
            .expect("no metadata")
            .annotations
            .expect("no annotations found");
        let restarted_at_annotation = annot.get("kube.kubernetes.io/restartedAt");
        assert!(
            restarted_at_annotation.is_some(),
            "StatefulSet was not restarted."
        );

        // ADD AN EXTENSION - ASSERT IT MAKES IT TO STATUS.EXTENSIONS
        // conductor receives a CRUDevent from control plane
        // take note of number of extensions at this point in time
        let mut extensions_add = extensions.clone();
        extensions_add.push(Extension {
            name: "pgmq".to_owned(),
            description: Some("pgmq description".to_string()),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                version: Some("0.2.1".to_owned()),
                schema: "public".to_owned(),
                database: "postgres".to_owned(),
            }],
        });
        let num_expected_extensions = extensions_add.len();
        let spec_js = serde_json::json!({
            "extensions": extensions_add,
        });
        let spec: CoreDBSpec = serde_json::from_value(spec_js).unwrap();
        let msg = types::CRUDevent {
            organization_name: org_name.clone(),
            data_plane_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            event_id: "test-install-extension".to_owned(),
            event_type: types::Event::Update,
            dbname: dbname.clone(),
            spec: Some(spec),
        };
        let msg_id = queue.send(&myqueue, &msg).await;
        println!("msg_id: {msg_id:?}");

        // read message from data_plane_events queue
        let msg = get_dataplane_message(retries, retry_delay, &queue).await;
        queue
            .archive("myqueue_data_plane", msg.msg_id)
            .await
            .expect("error deleting message");

        let extensions = msg
            .message
            .spec
            .expect("No spec found in message")
            .extensions;
        // we added an extension, so it should be +1 now
        assert_eq!(num_expected_extensions, extensions.len());

        // delete the instance
        let msg = types::CRUDevent {
            organization_name: org_name.clone(),
            data_plane_id: "org_02s3owPQskuGXHE8vYsGSY".to_owned(),
            event_id: "test-install-extension".to_owned(),
            event_type: types::Event::Delete,
            dbname: dbname.clone(),
            spec: None,
        };
        let msg_id = queue.send(&myqueue, &msg).await;
        println!("msg_id: {msg_id:?}");

        // wait for it to delete
        let wait_for_delete = 60;
        println!("Waiting {} seconds for delete operation", wait_for_delete);
        thread::sleep(time::Duration::from_secs(wait_for_delete));

        // assert namespace is gone
        let ns_api: Api<Namespace> = Api::all(client.clone());
        let ns_dne = ns_api.get(&namespace).await;
        assert!(ns_dne.is_err(), "Namespace was not deleted");
        // assert pvcs is gone
        let pvcs: Api<PersistentVolumeClaim> = Api::all(client.clone());
        let lp = ListParams::default().fields(&format!("metadata.name=data-{}-0", namespace));
        let pvc_list = pvcs.list(&lp).await.expect("failed to list pvcs");
        assert!(
            pvc_list.items.is_empty(),
            "PVCs were not deleted: {:?}",
            pvc_list
        );

        let cdb_api: Api<CoreDB> = Api::all(client.clone());
        let cdb_dne = cdb_api.get(&namespace).await;
        assert!(cdb_dne.is_err(), "CoreDB was not deleted");

        // call aws api and verify CF stack was deleted
        use aws_sdk_cloudformation::config::Region;
        use conductor::aws::cloudformation::AWSConfigState;
        let aws_region = "us-east-1".to_owned();
        let region = Region::new(aws_region);
        let aws_config_state = AWSConfigState::new(region).await;
        let stack_name = format!("org-{}-inst-{}-cf", org_name, dbname);
        let exists = aws_config_state.does_stack_exist(&stack_name).await;
        assert!(!exists, "CF stack was not deleted");
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
}
