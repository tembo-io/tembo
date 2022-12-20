// Include the #[ignore] macro on slow tests.
// That way, 'cargo test' does not run them by default.
// To run just these tests, use 'cargo test -- --ignored'
// To run all tests, use 'cargo test -- --include-ignored'
//
// https://doc.rust-lang.org/book/ch11-02-running-tests.html

#[tokio::test]
#[ignore]
async fn it_is() {
    // Initialize the Kubernetes client
    let _client = kube_client().await;
    assert!(true);
}

async fn kube_client() -> kube::Client {
    use k8s_openapi::api::core::v1::Namespace;
    use kube::{api::Api, Client, Config};

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

    // Set the labels you want to filter by
    // let mut params = ListParams::default();
    // params.labels("safe-to-run-coredb-testing=true");

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
    client
}
