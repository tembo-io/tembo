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
    use futures::TryStreamExt;
    use k8s_openapi::api::apps::v1::StatefulSet;
    use k8s_openapi::api::core::v1::Namespace;
    use kube::api::{ListParams, Patch, PatchParams};
    use kube::runtime::{watcher, WatchStreamExt};
    use kube::{Api, Client, Config};

    #[tokio::test]
    #[ignore]
    async fn functional_test_basic_create() {
        // Initialize the Kubernetes client
        let client = kube_client().await;

        // Configurations
        let name = "sample-coredb";
        let namespace = "default";
        let api_version = "kube.rs/v1";
        let kind = "CoreDB";
        let replicas = 1;

        // Timeout setting on 'watch' commands
        let watch_timeout_seconds = 20;

        // Apply a basic configuration of CoreDB
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": api_version,
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
        let coredb_resource = coredbs.patch(name, &params, &patch).await;

        // Check an STS was created

        let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
        let list_parameters = ListParams::default()
            .fields(&format!("metadata.name={}", name))
            .timeout(watch_timeout_seconds);
        //
        // https://docs.rs/kube-runtime/0.77.0/kube_runtime/watcher/fn.watcher.html
        //
        watcher(statefulsets, list_parameters)
            .applied_objects()
            .try_for_each(|p| async move {
                println!("Applied: {:?}", p);
                Ok(())
            })
            .await;
        //  Pod labels:
        //
        //	labels:
        //	  app: coredb
        //	  controller-revision-hash: sample-coredb-6f67db7fcb
        //	  statefulset.kubernetes.io/pod-name: sample-coredb-0
        //
        // STS labels:
        //
        //   labels:
        //   	app: coredb
        //
        // t 'Ok(CoreDB { metadata: ObjectMeta { annotations: None, cluster_name: None, creation_timestamp: Some(Time(2022-12-20T22:57:46Z)), deletion_grace_period_seconds: None, deletion_timestamp: None, finalizers: None, generate_name: None, generation: Some(1), labels: None, managed_fields: Some([ManagedFieldsEntry { api_version: Some("kube.rs/v1"), fields_type: Some("FieldsV1"), fields_v1: Some(FieldsV1(Object {"f:spec": Object {"f:replicas": Object {}}})), manager: Some("coredb-integration-test"), operation: Some("Apply"), subresource: None, time: Some(Time(2022-12-20T22:57:46Z)) }]), name: Some("sample-coredb"), namespace: Some("default"), owner_references: None, resource_version: Some("60907"), self_link: None, uid: Some("b4ad5f1e-5534-4bd4-811a-d0f793030b87") }, spec: CoreDBSpec { replicas: 1 }, status: None })'
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
            "expected to find label 'safe-to-run-core-db-tests'"
        );
        assert_eq!(
            labels["safe-to-run-coredb-tests"], "true",
            "expected to find label 'safe-to-run-core-db-tests' with value 'true'"
        );
        return client;
    }

    // fn shell_command(str: command) {
    // 	let output = Command::new("sh")
    // 	    .arg("-c")
    // 	    .arg(command)
    // 	    .stdout(Stdio::inherit())
    // 			.stderr(Stdio::inherit())
    // 	    .output()
    // 	    .expect("failed to run")
    // }
}
