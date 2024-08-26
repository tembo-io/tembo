// Include the #[gnore] macro on slow tests
// That way, 'cargo test' does not run them by default.
// To run just these tests, use 'cargo test -- --ignored'
// To run all tests, use 'cargo test -- --include-ignored'
//
// https://doc.rust-lang.org/book/ch11-02-running-tests.html
//
// These tests assume there is already kubernetes running and you have a context configured.
// It also assumes that the CRD(s) and operator are already installed for this cluster.
// In this way, it can be used as a conformance test on a target, separate from installation.
//
// Do your best to keep the function names as unique as possible.  This will help with
// debugging and troubleshooting and also Rust seems to match like named tests and will run them
// at the same time.  This can cause issues if they are not independent.

#[cfg(test)]
mod test {
    use anyhow::{Error as AnyError, Result};
    use chrono::{DateTime, SecondsFormat, Utc};
    use controller::{
        apis::coredb_types::CoreDB,
        cloudnativepg::{backups::Backup, clusters::Cluster},
        defaults::{default_resources, default_storage},
        errors::ValueError,
        is_pod_ready,
        psql::PsqlOutput,
        Context, State,
    };
    use futures_util::StreamExt;
    use k8s_openapi::{
        api::{
            apps::v1::Deployment,
            core::v1::{
                Namespace, PersistentVolumeClaim, Pod, ResourceRequirements, Secret, Service,
            },
        },
        apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
        apimachinery::pkg::{api::resource::Quantity, util::intstr::IntOrString},
    };
    use kube::{
        api::{
            AttachParams, DeleteParams, ListParams, Patch, PatchParams, WatchEvent, WatchParams,
        },
        runtime::wait::{await_condition, conditions, Condition},
        Api, Client, Config, Error,
    };
    use rand::Rng;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::{
        collections::{BTreeMap, BTreeSet},
        ops::Not,
        str,
        sync::Arc,
        thread,
        time::Duration,
    };

    use tokio::{io::AsyncReadExt, time::timeout};

    const API_VERSION: &str = "coredb.io/v1alpha1";
    // Timeout settings while waiting for an event
    const TIMEOUT_SECONDS_START_POD: u64 = 600;
    const TIMEOUT_SECONDS_POD_READY: u64 = 600;
    const TIMEOUT_SECONDS_NS_DELETED: u64 = 300;
    const TIMEOUT_SECONDS_POD_DELETED: u64 = 300;
    const TIMEOUT_SECONDS_COREDB_DELETED: u64 = 300;

    /// Struct to contain many commonly used test resources
    ///
    /// Most if not all tests use all of the fields here, or refer to them
    /// in some manner. This struct helps combine everything together.
    struct TestCore {
        name: String,
        namespace: String,
        client: Client,
        context: Arc<Context>,
        pods: Api<Pod>,
        coredbs: Api<CoreDB>,
        poolers: Api<Pooler>,
    }

    /// Helper class to make writing tests easier / less messy
    ///
    /// This class implements several functions for the TestClass struct that
    /// remove a lot of the boilerplate code that happens frequently in these
    /// tests. Use it whenever possible and feel free to add methods that
    /// should be listed.
    impl TestCore {
        /// Instantiate a new TestClass object
        ///
        /// By providing a test name, this function will return a TestClass
        /// object and set all of the related struct values as such:
        ///
        ///   * name - Test name as passed plus an RNG suffix
        ///   * namespace - An initialized Kubernetes namespace for the test
        ///   * client - An active Kubernetes client runtime
        ///   * context - An active Arc context
        ///   * pods - A pod to use for cluster-commands in this namespace and client
        ///   * coredbs - A CoreDB API tied to this namespace and client
        async fn new(test_name: &str) -> Self {
            let client = kube_client().await;
            let state = State::default();
            let context = state.create_context(client.clone());

            let mut rng = rand::thread_rng();
            let suffix = rng.gen_range(0..100000);
            let name = format!("{}-{}", test_name, suffix);
            let namespace = match create_namespace(client.clone(), &name).await {
                Ok(namespace) => namespace,
                Err(e) => {
                    eprintln!("Error creating namespace: {}", e);
                    std::process::exit(1);
                }
            };

            // Create a pod we can use to run commands in the cluster
            let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

            // Apply a basic configuration of CoreDB
            println!("Creating CoreDB resource {}", name);
            let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);

            let poolers: Api<Pooler> = Api::namespaced(client.clone(), &namespace);

            Self {
                name,
                namespace,
                client,
                context,
                pods,
                coredbs,
                poolers,
            }
        }

        /// Transform a JSON object into a cluster definition
        ///
        /// Given a series of JSON values defining a CoreDB cluster, this
        /// function will return a CoreDB object in the generated namespace.
        /// Subsequent calls will patch the existing cluster associated with
        /// the base object.
        async fn set_cluster_def(&self, cluster_def: &serde_json::Value) -> CoreDB {
            let params = PatchParams::apply("tembo-integration-test");
            let patch = Patch::Apply(&cluster_def);
            self.coredbs
                .patch(&self.name, &params, &patch)
                .await
                .unwrap()
        }

        // Tear down the test cluster, namespace, and other related allocations
        //
        // Once a test is finished, we should remove all structures we created.
        // Always call this function at the end of a test, and it will remove
        // the namespace for the test and all contained objects.
        async fn teardown(&self) {
            self.coredbs
                .delete(&self.name, &Default::default())
                .await
                .unwrap();
            println!("Waiting for CoreDB to be deleted: {}", &self.name);
            let _assert_coredb_deleted = tokio::time::timeout(
                Duration::from_secs(TIMEOUT_SECONDS_COREDB_DELETED),
                await_condition(self.coredbs.clone(), &self.name, conditions::is_deleted("")),
            )
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "CoreDB {} was not deleted after waiting {} seconds",
                    &self.name, TIMEOUT_SECONDS_COREDB_DELETED
                )
            });
            println!("CoreDB resource deleted {}", &self.name);

            // Delete namespace
            let _ = delete_namespace(self.client.clone(), &self.namespace).await;
        }
    }

    async fn kube_client() -> Client {
        // Get the name of the currently selected namespace
        let kube_config = Config::infer()
            .await
            .expect("Please configure your Kubernetes context.");
        let selected_namespace = &kube_config.default_namespace;

        // Initialize the Kubernetes client
        let client =
            Client::try_from(kube_config.clone()).expect("Failed to initialize Kubernetes client");

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

    async fn run_command_in_container(
        pods_api: Api<Pod>,
        pod_name: String,
        command: Vec<String>,
        container: Option<String>,
    ) -> String {
        let attach_params = AttachParams {
            container: container.clone(),
            tty: false,
            stdin: true,
            stdout: true,
            stderr: true,
            max_stdin_buf_size: Some(1024),
            max_stdout_buf_size: Some(1024),
            max_stderr_buf_size: Some(1024),
        };

        let max_retries = 10;
        let millisec_between_tries = 5;

        for _i in 1..max_retries {
            let attach_res = pods_api
                .exec(pod_name.as_str(), &command, &attach_params)
                .await;
            let mut attached_process = match attach_res {
                Ok(ap) => ap,
                Err(e) => {
                    println!(
                        "Error attaching to pod: {}, container: {:?}, error: {}",
                        pod_name, container, e
                    );
                    thread::sleep(Duration::from_millis(millisec_between_tries));
                    continue;
                }
            };
            let mut stdout_reader = attached_process.stdout().unwrap();
            let mut result_stdout = String::new();
            stdout_reader
                .read_to_string(&mut result_stdout)
                .await
                .unwrap();

            return result_stdout;
        }
        panic!("Failed to run command in container");
    }

    async fn psql_with_retry(
        context: Arc<Context>,
        coredb_resource: CoreDB,
        query: String,
    ) -> PsqlOutput {
        for _ in 1..40 {
            // Assert extension no longer created
            if let Ok(result) = coredb_resource
                .psql(query.clone(), "postgres".to_string(), context.clone())
                .await
            {
                if result.stdout.is_some() {
                    return result;
                }
            }
            println!(
                "Waiting for psql result on DB {}...",
                coredb_resource.metadata.name.clone().unwrap()
            );
            thread::sleep(Duration::from_millis(5000));
        }
        panic!("Timed out waiting for psql result of '{}'", query);
    }

    async fn http_get_with_retry(
        url: &str,
        headers: Option<BTreeMap<String, String>>,
        retries: usize,
        delay: usize,
    ) -> Result<reqwest::Response> {
        let mut headers_map = HeaderMap::new();
        if let Some(h) = headers {
            for (key, value) in h {
                let header_name = HeaderName::from_bytes(key.as_bytes()).unwrap();
                let header_value = HeaderValue::from_str(&value).unwrap();
                headers_map.insert(header_name, header_value);
            }
        };

        let httpclient = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .default_headers(headers_map)
            .build()
            .unwrap();
        println!("Sending request to '{}'", url);
        for i in 1..retries {
            let response = httpclient.get(url).send().await;
            if response.is_err() {
                tokio::time::sleep(Duration::from_secs(delay as u64)).await;
                println!(
                    "Retry {}/{} request -- error: {}",
                    i,
                    retries,
                    response.err().unwrap()
                );
            } else {
                let resp = response.unwrap();
                if resp.status() == 200 {
                    return Ok(resp);
                } else {
                    tokio::time::sleep(Duration::from_secs(delay as u64)).await;
                    println!(
                        "Retry {}/{} request -- status: {}",
                        i,
                        retries,
                        resp.status()
                    );
                }
            }
        }
        Err(AnyError::msg(format!(
            "Timed out waiting for http response from '{}'",
            url
        )))
    }

    async fn wait_until_psql_contains(
        context: Arc<Context>,
        coredb_resource: CoreDB,
        query: String,
        expected: String,
        inverse: bool,
    ) -> PsqlOutput {
        // Wait up to 200 seconds
        for _ in 1..40 {
            thread::sleep(Duration::from_millis(5000));
            // Assert extension no longer created
            let result = coredb_resource
                .psql(query.clone(), "postgres".to_string(), context.clone())
                .await;
            if let Ok(output) = result {
                match inverse {
                    true => {
                        if !output
                            .stdout
                            .clone()
                            .unwrap()
                            .contains(expected.clone().as_str())
                        {
                            return output;
                        }
                    }
                    false => {
                        if output
                            .stdout
                            .clone()
                            .unwrap()
                            .contains(expected.clone().as_str())
                        {
                            return output;
                        }
                    }
                }
            }
            println!(
                "Waiting for psql result on DB {}...",
                coredb_resource.metadata.name.clone().unwrap()
            );
        }
        if inverse {
            panic!(
                "Timed out waiting for psql result of '{}' to not contain {}",
                query, expected
            );
        }
        panic!(
            "Timed out waiting for psql result of '{}' to contain {}",
            query, expected
        );
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

    pub fn is_backup_completed() -> impl Condition<Backup> + 'static {
        move |obj: Option<&Backup>| {
            if let Some(backup) = &obj {
                if let Some(status) = &backup.status {
                    if status.phase.as_deref() == Some("completed") {
                        return true;
                    }
                }
            }
            false
        }
    }

    async fn has_backup_completed(context: Arc<Context>, namespace: &str, name: &str) {
        println!("Waiting for backup to complete: {}", name);
        let backups: Api<Backup> = Api::namespaced(context.client.clone(), namespace);
        let lp = ListParams::default().labels(&format!("cnpg.io/cluster={}", name));

        const TIMEOUT_SECONDS_BACKUP_COMPLETED: u64 = 300;

        let start_time = std::time::Instant::now();

        loop {
            let backup_result = backups.list(&lp).await;
            let mut backup_completed = false;
            if let Ok(backup_list) = backup_result {
                for backup in backup_list.items {
                    if let Some(backup_name) = &backup.metadata.name {
                        println!("Found backup: {}", backup_name);
                        if await_condition(backups.clone(), backup_name, is_backup_completed())
                            .await
                            .is_ok()
                        {
                            backup_completed = true;
                            break;
                        }
                    } else {
                        println!("Found backup with no name");
                    }
                }
            } else {
                println!("Backup {} not found, retrying...", name);
            }

            if backup_completed {
                println!("Backup is complete: {}", name);
                break;
            }

            // Check the elapsed time and break the loop if it's more than your overall timeout
            if start_time.elapsed() > Duration::from_secs(TIMEOUT_SECONDS_BACKUP_COMPLETED) {
                println!(
                    "Failed to find completed backup {} after waiting {} seconds",
                    name, TIMEOUT_SECONDS_BACKUP_COMPLETED
                );
                break;
            }

            // Sleep for a short duration before retrying
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn service_exists(
        context: Arc<Context>,
        namespace: &str,
        service_name: &str,
        inverse: bool,
    ) -> Option<Service> {
        println!(
            "Checking for service existence: {}, inverse: {}",
            service_name, inverse
        );
        let services: Api<Service> = Api::namespaced(context.client.clone(), namespace);

        const TIMEOUT_SECONDS_SERVICE_CHECK: u64 = 300;
        let start_time = std::time::Instant::now();

        loop {
            match services.get(service_name).await {
                Ok(service) => {
                    if inverse {
                        println!("Service {} should not exist, but it does", service_name);
                        return Some(service);
                    } else {
                        println!("Service {} exists", service_name);
                        return Some(service);
                    }
                }
                Err(_) => {
                    if inverse {
                        return None;
                    } else {
                        println!("Service {} not found, retrying...", service_name);
                    }
                }
            }

            if start_time.elapsed() > Duration::from_secs(TIMEOUT_SECONDS_SERVICE_CHECK) {
                println!(
                    "Failed to find service {} after waiting {} seconds",
                    service_name, TIMEOUT_SECONDS_SERVICE_CHECK
                );

                if let Ok(service_list) = services.list(&ListParams::default()).await {
                    println!("Services in namespace {}:", namespace);
                    for service in service_list.items {
                        println!(
                            "Service: {}, Labels: {:?}",
                            service.metadata.name.unwrap_or_default(),
                            service.metadata.labels.unwrap_or_default()
                        );
                    }
                } else {
                    println!("Failed to list services in namespace {}", namespace);
                }

                break;
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        None
    }

    // Create namespace for the test to run in
    async fn create_namespace(client: Client, name: &str) -> Result<String, Error> {
        let ns_api: Api<Namespace> = Api::all(client);
        let params = ListParams::default().fields(&format!("metadata.name={}", name));
        let ns_list = ns_api.list(&params).await.unwrap();
        if !ns_list.items.is_empty() {
            return Ok(name.to_string());
        }

        println!("Creating namespace {}", name);
        let params = PatchParams::apply("tembo-integration-tests");
        let ns = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Namespace",
            "metadata": {
                "name": name,
                "labels": {
                    "tembo-pod-init.tembo.io/watch": "true",
                    "safe-to-run-coredb-tests": "true",
                    "kubernetes.io/metadata.name": name
                }
            }
        });
        let _o = ns_api.patch(name, &params, &Patch::Apply(ns)).await?;

        // return the name of the namespace
        Ok(name.to_string())
    }

    // Delete namespace
    async fn delete_namespace(client: Client, name: &str) -> Result<(), Error> {
        let ns_api: Api<Namespace> = Api::all(client);
        let params = ListParams::default().fields(&format!("metadata.name={}", name));
        let ns_list = ns_api.list(&params).await?;
        if ns_list.items.is_empty() {
            return Ok(());
        }

        println!("Deleting namespace {}", name);
        let params = DeleteParams::default();
        let _o = ns_api.delete(name, &params).await?;

        Ok(())
    }

    async fn wait_until_status_not_running(
        coredbs: &Api<CoreDB>,
        name: &str,
    ) -> Result<(), kube::Error> {
        const TIMEOUT_SECONDS_STATUS_RUNNING: u32 = 294;
        let wp = WatchParams {
            timeout: Some(TIMEOUT_SECONDS_STATUS_RUNNING),
            field_selector: Some(format!("metadata.name={}", name)),
            ..Default::default()
        };
        let mut stream = coredbs.watch(&wp, "0").await?.boxed();

        let result = timeout(Duration::from_secs(300), async {
            while let Some(status) = stream.next().await {
                match status {
                    Ok(WatchEvent::Modified(cdb)) => {
                        let running_status = cdb.status.as_ref().map_or(false, |s| s.running);
                        if !running_status {
                            println!("status.running is now false!");
                            return Ok(());
                        } else {
                            println!("status.running is still true. Continuing to watch...");
                        }
                    }
                    Ok(_) => {} // You might want to handle other events such as Error or Deleted
                    Err(e) => {
                        println!("Watch error: {:?}", e);
                    }
                }
            }
            Err(ValueError::Invalid(
                "Stream terminated prematurely".to_string(),
            ))
        })
        .await;

        match result {
            Ok(_ok) => Ok(()),
            Err(_) => Err(kube::Error::ReadEvents(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Timed out waiting for status.running to become false",
            ))),
        }
    }

    use controller::extensions::database_queries::LIST_EXTENSIONS_QUERY;
    use controller::{
        apis::postgres_parameters::{ConfigValue, PgConfig},
        cloudnativepg::poolers::Pooler,
        errors,
        ingress_route_crd::IngressRoute,
        traefik::ingress_route_tcp_crd::IngressRouteTCP,
    };
    use k8s_openapi::NamespaceResourceScope;
    use serde::{de::DeserializeOwned, Deserialize};

    async fn get_resource<R>(
        client: Client,
        namespace: &str,
        name: &str,
        retries: usize,
        // is resource expected to exist?
        expected: bool,
    ) -> Result<R, kube::Error>
    where
        R: kube::api::Resource<Scope = NamespaceResourceScope>
            + std::fmt::Debug
            + 'static
            + Clone
            + DeserializeOwned
            + for<'de> serde::Deserialize<'de>,
        R::DynamicType: Default,
    {
        let api: Api<R> = Api::namespaced(client, namespace);
        for _ in 0..retries {
            let resource = api.get(name).await;
            if expected {
                if resource.is_ok() {
                    return resource;
                } else {
                    println!("Failed to get resource: {}. Retrying...", name);
                    thread::sleep(Duration::from_millis(2000));
                }
            } else if resource.is_err() {
                return resource;
            } else {
                println!("Resource {} should not exist. Retrying...", name);
                thread::sleep(Duration::from_millis(2000));
            }
        }
        panic!("Timed out getting resource, {}", name);
    }
    // helper function retrieve all instances of a resource in namespace
    // used repeatedly in appService tests
    // handles retries
    async fn list_resources<R>(
        client: Client,
        cdb_name: &str,
        namespace: &str,
        num_expected: usize,
    ) -> Result<Vec<R>, errors::OperatorError>
    where
        R: kube::api::Resource<Scope = NamespaceResourceScope>
            + std::fmt::Debug
            + 'static
            + Clone
            + DeserializeOwned
            + for<'de> serde::Deserialize<'de>,
        R::DynamicType: Default,
    {
        let api: Api<R> = Api::namespaced(client, namespace);
        let lp = ListParams::default().labels(format!("coredb.io/name={}", cdb_name).as_str());
        let retry = 15;
        let mut passed_retry = false;
        let mut resource_list: Vec<R> = Vec::new();
        for _ in 0..retry {
            let resources = api.list(&lp).await?;
            if resources.items.len() == num_expected {
                resource_list.extend(resources.items);
                passed_retry = true;
                break;
            } else {
                println!(
                    "ns:{}.cdb:{} Found {}, expected {}",
                    namespace,
                    cdb_name,
                    resources.items.len(),
                    num_expected
                );
            }
            thread::sleep(Duration::from_millis(2000));
        }
        if passed_retry {
            Ok(resource_list)
        } else {
            Err(
                errors::ValueError::Invalid("Failed to get all resources in namespace".to_string())
                    .into(),
            )
        }
    }

    // function to check coredb.status.trunk_installs status of specific extension
    async fn trunk_install_status(coredbs: &Api<CoreDB>, name: &str, extension: &str) -> bool {
        let max_retries = 10;
        let wait_duration = Duration::from_secs(6); // Adjust as needed

        for attempt in 1..=max_retries {
            match coredbs.get(name).await {
                Ok(coredb) => {
                    let has_extension_without_error = coredb.status.as_ref().map_or(false, |s| {
                        s.trunk_installs.as_ref().map_or(false, |installs| {
                            installs
                                .iter()
                                .any(|install| install.name == extension && !install.error)
                        })
                    });

                    if has_extension_without_error {
                        println!(
                            "CoreDB {} has trunk_install status for {} without error",
                            name, extension
                        );
                        return true;
                    } else {
                        println!(
                                "Attempt {}/{}: CoreDB {} does not have trunk_install status for {} or has an error",
                                attempt, max_retries, name, extension
                            );
                    }
                }
                Err(e) => {
                    println!(
                        "Failed to get CoreDB on attempt {}/{}: {}",
                        attempt, max_retries, e
                    );
                }
            }

            tokio::time::sleep(wait_duration).await;
        }

        println!(
            "CoreDB {} did not have trunk_install status for {} without error after {} attempts",
            name, extension, max_retries
        );
        false
    }

    // Wait for a specific extension to be enabled in coredb.status.extensions. Check for disabled with inverse value.
    async fn wait_for_extension_status_enabled(
        coredbs: &Api<CoreDB>,
        name: &str,
        extension: &str,
        inverse: bool,
    ) -> Result<(), kube::Error> {
        let max_retries = 10;
        let wait_duration = Duration::from_secs(2); // Adjust as needed

        for attempt in 1..=max_retries {
            match coredbs.get(name).await {
                Ok(coredb) => {
                    // Check if the extension is enabled in the status
                    let has_extension = coredb.status.as_ref().map_or(false, |s| {
                        s.extensions.as_ref().map_or(false, |extensions| {
                            extensions.iter().any(|ext| {
                                ext.name == extension
                                    && ext.locations.iter().any(|loc| {
                                        loc.enabled.unwrap() && loc.database == "postgres"
                                    })
                            })
                        })
                    });

                    if inverse {
                        if !has_extension {
                            println!(
                                "CoreDB {} has extension {} disabled in status",
                                name, extension
                            );
                            return Ok(());
                        } else {
                            println!(
                                "Attempt {}/{}: CoreDB {} has extension {} enabled in status",
                                attempt, max_retries, name, extension
                            );
                        }
                    } else if has_extension {
                        println!(
                            "CoreDB {} has extension {} enabled in status",
                            name, extension
                        );
                        return Ok(());
                    } else {
                        println!(
                            "Attempt {}/{}: CoreDB {} has extension {} disabled in status",
                            attempt, max_retries, name, extension
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "Failed to get CoreDB on attempt {}/{}: {}",
                        attempt, max_retries, e
                    );
                }
            }

            tokio::time::sleep(wait_duration).await;
        }

        if inverse {
            println!(
                "CoreDB {} did not have extension {} disabled in status after {} attempts",
                name, extension, max_retries
            );
        } else {
            println!(
                "CoreDB {} did not have extension {} enabled in status after {} attempts",
                name, extension, max_retries
            );
        }
        Err(kube::Error::ReadEvents(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Timed out waiting for extension to be enabled",
        )))
    }

    // Function to wait for metrics to appear
    async fn wait_for_metric(
        pods: Api<Pod>,
        pod_name: String,
        metric_name: &str,
    ) -> Result<String, String> {
        let max_retries = 15; // Adjust as needed
        let wait_duration = Duration::from_secs(2);

        for attempt in 1..=max_retries {
            let command = vec![
                String::from("curl"),
                "http://localhost:9187/metrics".to_string(),
            ];
            let result_stdout = run_command_in_container(
                pods.clone(),
                pod_name.clone(),
                command,
                Some("postgres".to_string()),
            )
            .await;

            // Check if the result contains the expected metric
            if result_stdout.contains(metric_name) {
                return Ok(result_stdout);
            }

            println!(
                "Attempt {}/{}: Metric '{}' not found in output.",
                attempt, max_retries, metric_name
            );

            tokio::time::sleep(wait_duration).await;
        }

        Err(format!(
            "Metric '{}' not found after {} attempts",
            metric_name, max_retries
        ))
    }
    async fn wait_til_status_is_filled(coredbs: &Api<CoreDB>, name: &str) {
        let max_retries = 10; // adjust as needed
        for attempt in 1..=max_retries {
            let coredb = coredbs
                .get(name)
                .await
                .unwrap_or_else(|_| panic!("Failed to get CoreDB: {}", name));

            if coredb.status.is_some() {
                println!("Status is filled for CoreDB: {}", name);
                return;
            } else {
                println!(
                    "Attempt {}/{}: Status not yet filled for CoreDB: {}",
                    attempt, max_retries, name
                );
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        panic!(
            "Failed to fetch filled status for CoreDB: {} after {} attempts",
            name, max_retries
        );
    }

    async fn get_pg_start_time(
        coredbs: &Api<CoreDB>,
        name: &str,
        ctx: Arc<Context>,
    ) -> DateTime<Utc> {
        const PG_TIMESTAMP_DECL: &str = "%Y-%m-%d %H:%M:%S.%f%#z";

        let coredb = coredbs.get(name).await.expect("spec not found");

        let query = "SELECT pg_postmaster_start_time()".to_string();
        let psql_output = psql_with_retry(ctx.clone(), coredb, query).await;
        let stdout = psql_output
            .stdout
            .as_ref()
            .and_then(|stdout| stdout.lines().nth(2).map(str::trim))
            .expect("expected stdout");

        DateTime::parse_from_str(stdout, PG_TIMESTAMP_DECL)
            .unwrap()
            .into()
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_basic_cnpg() {
        let test_name = "test-basic-cnpg";
        let test = TestCore::new(test_name).await;
        let name = test.name.clone();

        let kind = "CoreDB";
        let replicas = 1;

        // Generate basic CoreDB resource to start with
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "extensions": [{
                        // Try including an extension
                        // without specifying a schema
                        "name": "pg_jsonschema",
                        "description": "fake description",
                        "locations": [{
                            "enabled": true,
                            "version": "0.1.4",
                            "database": "postgres",
                        }],
                    }],
                "trunk_installs": [{
                        "name": "pg_jsonschema",
                        "version": "0.1.4",
                }]
            }
        });

        let coredb_resource = test.set_cluster_def(&coredb_json).await;

        // Wait for CNPG Pod to be created
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(test.pods.clone(), pod_name.clone()).await;

        let _ = wait_until_psql_contains(
            test.context.clone(),
            coredb_resource.clone(),
            "\\dx".to_string(),
            "pg_jsonschema".to_string(),
            false,
        )
        .await;

        // Wait for pg_jsonschema to be installed before proceeding.
        let found_extension = trunk_install_status(&test.coredbs, &name, "pg_jsonschema").await;
        assert!(found_extension);

        // Check for heartbeat table and values
        let sql_result = wait_until_psql_contains(
            test.context.clone(),
            coredb_resource.clone(),
            "SELECT latest_heartbeat FROM tembo.heartbeat_table LIMIT 1".to_string(),
            "postgres".to_string(),
            true,
        )
        .await;
        assert!(sql_result.success);

        let cdb_name = coredb_resource.metadata.name.clone().unwrap();
        let metrics_url = format!("https://{}.localhost:8443/metrics", cdb_name);
        let response = http_get_with_retry(&metrics_url, None, 100, 5)
            .await
            .unwrap();
        let response_code = response.status();
        assert!(response_code.is_success());
        let body = response.text().await.unwrap();
        assert!(body.contains("cnpg_pg_settings_setting"));

        test.teardown().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_networking() {
        let client = kube_client().await;
        let state = State::default();

        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-networking-{}", suffix.clone());
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };
        let kind = "CoreDB";
        let replicas = 2;

        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        println!("Creating CoreDB resource {}", name);
        let _test_metric_decr = format!("coredb_integration_test_{}", suffix.clone());
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
            }
        });
        let params = PatchParams::apply("functional-test-networking");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        let pod_name = format!("{}-1", name);
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

        let ing_route_tcp_name = format!("{}-rw-0", name);
        let ingress_route_tcp_api: Api<IngressRouteTCP> =
            Api::namespaced(client.clone(), &namespace);
        let ing_route_tcp = ingress_route_tcp_api
            .get(&ing_route_tcp_name)
            .await
            .unwrap_or_else(|_| {
                panic!("Expected to find ingress route TCP {}", ing_route_tcp_name)
            });
        let service_name = ing_route_tcp.spec.routes[0]
            .services
            .clone()
            .expect("Ingress route has no services")[0]
            .name
            .clone();
        assert_eq!(&service_name, format!("{}-rw", name).as_str());

        let ing_route_tcp_name = format!("{}-ro-0", name);
        let ingress_route_tcp_api: Api<IngressRouteTCP> =
            Api::namespaced(client.clone(), &namespace);
        let ing_route_tcp = ingress_route_tcp_api
            .get(&ing_route_tcp_name)
            .await
            .unwrap_or_else(|_| {
                panic!("Expected to find ingress route TCP {}", ing_route_tcp_name)
            });
        let service_name = ing_route_tcp.spec.routes[0]
            .services
            .clone()
            .expect("Ingress route has no services")[0]
            .name
            .clone();
        assert_eq!(&service_name, format!("{}-ro", name).as_str());

        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "extra_domains_rw": ["any-given-domain.com", "another-domain.com"]
            }
        });
        let params = PatchParams::apply("functional-test-networking");
        let patch = Patch::Merge(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        tokio::time::sleep(Duration::from_secs(5)).await;

        let ing_route_tcp_name = format!("extra-{}-rw", name);
        let ingress_route_tcp_api: Api<IngressRouteTCP> =
            Api::namespaced(client.clone(), &namespace);
        let ing_route_tcp = ingress_route_tcp_api
            .get(&ing_route_tcp_name)
            .await
            .unwrap_or_else(|_| {
                panic!("Expected to find ingress route TCP {}", ing_route_tcp_name)
            });
        let service_name = ing_route_tcp.spec.routes[0]
            .services
            .clone()
            .expect("Ingress route has no services")[0]
            .name
            .clone();
        assert_eq!(&service_name, format!("{}-rw", name).as_str());
        let matcher = ing_route_tcp.spec.routes[0].r#match.clone();
        assert_eq!(
            matcher,
            "HostSNI(`another-domain.com`) || HostSNI(`any-given-domain.com`)"
        );

        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "extra_domains_rw": ["new-domain.com"]
            }
        });
        let params = PatchParams::apply("functional-test-networking");
        let patch = Patch::Merge(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        tokio::time::sleep(Duration::from_secs(5)).await;

        let ing_route_tcp = ingress_route_tcp_api
            .get(&ing_route_tcp_name)
            .await
            .unwrap_or_else(|_| {
                panic!("Expected to find ingress route TCP {}", ing_route_tcp_name)
            });
        let service_name = ing_route_tcp.spec.routes[0]
            .services
            .clone()
            .expect("Ingress route has no services")[0]
            .name
            .clone();
        assert_eq!(&service_name, format!("{}-rw", name).as_str());
        let matcher = ing_route_tcp.spec.routes[0].r#match.clone();
        assert_eq!(matcher, "HostSNI(`new-domain.com`)");

        let middlewares = ing_route_tcp.spec.routes[0].middlewares.clone().unwrap();
        assert_eq!(middlewares.len(), 1);

        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "extra_domains_rw": [],
            }
        });
        let params = PatchParams::apply("functional-test-networking").force();
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        let mut i = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let ing_route_tcp = ingress_route_tcp_api.get(&ing_route_tcp_name).await;
            if i > 5 || ing_route_tcp.is_err() {
                break;
            }
            i += 1;
        }
        let ing_route_tcp = ingress_route_tcp_api.get(&ing_route_tcp_name).await;
        assert!(ing_route_tcp.is_err());

        // Enable Dedicated Networking Test
        let context = state.create_context(client.clone());

        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "dedicatedNetworking": {
                    "enabled": true,
                    "includeStandby": true,
                    "public": true,
                    "serviceType": "LoadBalancer"
                }
            }
        });
        let params = PatchParams::apply("functional-test-dedicated-networking");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        tokio::time::sleep(Duration::from_secs(10)).await;

        let service_dedicated = service_exists(
            context.clone(),
            &namespace,
            &format!("{}-dedicated", name),
            false,
        )
        .await;
        let service_dedicated_ro = service_exists(
            context.clone(),
            &namespace,
            &format!("{}-dedicated-ro", name),
            false,
        )
        .await;

        assert!(service_dedicated.is_some());
        assert!(service_dedicated_ro.is_some());

        let service = service_dedicated.unwrap();
        assert_eq!(
            service.spec.as_ref().unwrap().type_,
            Some("ClusterIP".to_string())
        );
        assert_eq!(
            service
                .metadata
                .labels
                .as_ref()
                .expect("Labels should be present")
                .get("public")
                .expect("Public label should be present"),
            "true"
        );

        let service = service_dedicated_ro.unwrap();
        assert_eq!(
            service.spec.as_ref().unwrap().type_,
            Some("ClusterIP".to_string())
        );
        assert_eq!(
            service
                .metadata
                .labels
                .as_ref()
                .expect("Labels should be present")
                .get("public")
                .expect("Public label should be present"),
            "true"
        );

        // Disable dedicated networking
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "dedicatedNetworking": {
                    "enabled": false,
                }
            }
        });
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        tokio::time::sleep(Duration::from_secs(10)).await;

        let service_deleted = service_exists(
            context.clone(),
            &namespace,
            &format!("{}-dedicated", name),
            true,
        )
        .await
        .is_none();
        let service_ro_deleted = service_exists(
            context.clone(),
            &namespace,
            &format!("{}-dedicated-ro", name),
            true,
        )
        .await
        .is_none();

        assert!(service_deleted);
        assert!(service_ro_deleted);

        coredbs.delete(name, &Default::default()).await.unwrap();
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
        println!("CoreDB resource deleted {}", name);

        let _ = delete_namespace(client.clone(), &namespace).await;
    }
}
