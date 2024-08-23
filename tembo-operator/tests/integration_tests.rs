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
    async fn functional_test_basic_cnpg_assuming_latest_version() {
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
                        // without specifying a version
                        "name": "pg_jsonschema",
                        "description": "fake description",
                        "locations": [{
                            "enabled": true,
                            "database": "postgres",
                        }],
                    }],
                "trunk_installs": [{
                        "name": "pg_jsonschema"
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
    async fn functional_test_basic_cnpg_pg16() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-basic-cnpg-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                panic!("Error creating namespace: {}", e);
            }
        };

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": "CoreDB",
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": 1,
                "image": "quay.io/tembo/standard-cnpg:16-a0a5ab5",
                "extensions": [{
                        "name": "cube",
                        "description": "fake description",
                        "locations": [{
                            "enabled": true,
                            "version": "1.5",
                            "database": "postgres",
                        }],
                    }],
                "trunk_installs": [{
                        "name": "cube",
                        "version": "1.5.0",
                }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        let _ = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "\\dx".to_string(),
            "cube".to_string(),
            false,
        )
        .await;

        // Wait for cube to be installed before proceeding.
        let found_extension = trunk_install_status(&coredbs, name, "cube").await;
        assert!(found_extension);

        // Check for heartbeat table and values
        let sql_result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT latest_heartbeat FROM tembo.heartbeat_table LIMIT 1".to_string(),
            "postgres".to_string(),
            true,
        )
        .await;
        assert!(sql_result.success);

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_cnpg_metrics_create() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-cnpg-metrics-create-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };
        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let test_metric_decr = format!("coredb_integration_test_{}", rng.gen_range(0..100000));
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
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
                        "name": "aggs_for_vecs",
                        "description": "aggs_for_vecs extension",
                        "locations": [{
                            "enabled": true,
                            "version": "1.3.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }],
                "trunk_installs": [
                    {
                        "name": "aggs_for_vecs",
                        "version": "1.3.0",
                    }],
                "metrics": {
                    "enabled": true,
                    "queries": {
                        "test_ns": {
                            "query": "SELECT 10 as my_metric, 'cat' as animal",
                            "master": true,
                            "metrics": [
                              {
                                "my_metric": {
                                  "usage": "GAUGE",
                                  "description": test_metric_decr
                                }
                              },
                              {
                                "animal": {
                                    "usage": "LABEL",
                                    "description": "Animal type"
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

        // Wait for Pod to be created
        // This is the CNPG pod
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // Assert default storage values are applied to PVC
        let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), &namespace);
        let default_storage: Quantity = default_storage();

        // In CNPG, the PVC name is the same as the pod name
        let pvc = pvc_api.get(&pod_name.to_string()).await.unwrap();
        let storage = pvc.spec.unwrap().resources.unwrap().requests.unwrap();
        let s = storage.get("storage").unwrap().to_owned();
        assert_eq!(default_storage, s);

        // Assert default resource values are applied to postgres container
        let default_resources: ResourceRequirements = default_resources();
        let pg_pod = pods.get(&pod_name).await.unwrap();
        let resources = pg_pod.spec.unwrap().containers[0].clone().resources;
        assert_eq!(default_resources, resources.unwrap());

        // Assert no tables found
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dt".to_string()).await;
        println!("psql out: {}", result.stdout.clone().unwrap());
        assert!(!result.stdout.clone().unwrap().contains("customers"));

        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "
                CREATE TABLE customers (
                   id serial PRIMARY KEY,
                   name VARCHAR(50) NOT NULL,
                   email VARCHAR(50) NOT NULL UNIQUE,
                   created_at TIMESTAMP DEFAULT NOW()
                );
            "
            .to_string(),
        )
        .await;
        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("CREATE TABLE"));

        // Assert table 'customers' exists
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dt".to_string()).await;
        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("customers"));

        let result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select * from pg_extension;".to_string(),
            "aggs_for_vecs".to_string(),
            false,
        )
        .await;

        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("aggs_for_vecs"));

        // Check for metrics and availability
        let metric_name = format!("cnpg_collector_up{{cluster=\"{}\"}} 1", name);
        match wait_for_metric(pods.clone(), pod_name.to_string(), &metric_name).await {
            Ok(result_stdout) => {
                println!("Metric found: {}", result_stdout);
            }
            Err(e) => {
                panic!("Failed to find metric: {}", e);
            }
        }

        // Look for the custom metric
        match wait_for_metric(pods.clone(), pod_name.to_string(), &test_metric_decr).await {
            Ok(result_stdout) => {
                println!("Metric found: {}", result_stdout);
            }
            Err(e) => {
                panic!("Failed to find metric: {}", e);
            }
        }

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
                        "name": "aggs_for_vecs",
                        "description": "aggs_for_vecs extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.3.0",
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

        let result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "aggs_for_vecs".to_string(),
            true,
        )
        .await;

        assert!(
            !result.stdout.clone().unwrap().contains("aggs_for_vecs"),
            "results should not contain aggs_for_vecs: {}",
            result.stdout.clone().unwrap()
        );

        // assert extensions made it into the status
        let spec = coredbs.get(name).await.expect("spec not found");
        let status = spec.status.expect("no status on coredb");
        let extensions = status.extensions;
        assert!(!extensions.clone().expect("expected extensions").is_empty());
        assert!(!extensions.expect("expected extensions")[0]
            .description
            .clone()
            .expect("expected a description")
            .is_empty());

        // Change size of a PVC
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "storage": "10Gi",
            }
        });
        let params = PatchParams::apply("coredb-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _ = coredbs.patch(name, &params, &patch).await.unwrap();
        // Wait up to 60s for the storage size to change
        let mut i = 0;
        loop {
            thread::sleep(Duration::from_millis(10000));
            let pvc = pvc_api.get(&pod_name.to_string()).await.unwrap();
            let storage = pvc.spec.unwrap().resources.unwrap().requests.unwrap();
            let s = storage.get("storage").unwrap().to_owned();
            if i > 5 || s == Quantity("10Gi".to_owned()) {
                break;
            }
            i += 1;
        }
        let pvc = pvc_api.get(&pod_name.to_string()).await.unwrap();
        // checking that the request is set, but its not the status
        // https://github.com/rancher/local-path-provisioner/issues/323
        let storage = pvc.spec.unwrap().resources.unwrap().requests.unwrap();
        let s = storage.get("storage").unwrap().to_owned();
        assert_eq!(Quantity("10Gi".to_owned()), s);

        // Cleanup CoreDB resource
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

        // Delete namespace
        let _ = delete_namespace(client, &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_cnpg_pgparams() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-cnpg-pgparams-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate CoreDB resource with params
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "pg_partman",
                        "version": "4.7.3",
                    },
                    {
                        "name": "pgmq",
                        "version": "1.1.1",
                    },
                    {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                    },
                    {
                        "name": "postgis",
                        "version": "3.4.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "pg_partman",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "4.7.3",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "pgmq",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "1.1.1",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "pg_stat_statements",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "1.10.0",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "postgis",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "3.4.0",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    }
                ],
                "runtime_config": [
                    {
                        "name": "shared_preload_libraries",
                        "value": "pg_stat_statements"
                    },
                    {
                        "name": "pg_partman_bgw.interval",
                        "value": "60"
                    },
                    {
                        "name": "pg_partman_bgw.role",
                        "value": "postgres"
                    },
                    {
                        "name": "pg_partman_bgw.dbname",
                        "value": "postgres"
                    }
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        println!("Waiting to install extension pgmq");

        let result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "pgmq".to_string(),
            false,
        )
        .await;

        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("pgmq"));

        let result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "postgis".to_string(),
            false,
        )
        .await;

        println!("{}", result.stdout.clone().unwrap());
        assert!(result.stdout.clone().unwrap().contains("postgis"));

        println!("Restarting CNPG pod");
        // Restart the CNPG instance
        let cluster: Api<Cluster> = Api::namespaced(client.clone(), &namespace);
        let restart = Utc::now()
            .to_rfc3339_opts(SecondsFormat::Secs, true)
            .to_string();

        // To restart the CNPG pod we need to annotate the Cluster resource with
        // kubectl.kubernetes.io/restartedAt: <timestamp>
        let patch_json = serde_json::json!({
            "metadata": {
                "annotations": {
                    "kubectl.kubernetes.io/restartedAt": restart
                }
            }
        });

        // Use the patch method to update the Cluster resource
        let params = PatchParams::default();
        let patch = Patch::Merge(patch_json);
        let _patch = cluster.patch(name, &params, &patch);

        let _result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "show shared_preload_libraries;".to_string(),
            "pg_partman_bgw".to_string(),
            false,
        )
        .await;

        // Assert that shared_preload_libraries contains pg_stat_statements
        // and pg_partman_bgw

        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "show shared_preload_libraries;".to_string(),
        )
        .await;

        let stdout = match result.stdout {
            Some(output) => output,
            None => panic!("stdout is None"),
        };

        assert!(stdout.contains("pg_partman_bgw"));
        assert!(stdout.contains("pg_stat_statements"));

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_skip_reconciliation() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let _context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-skip-reconciliation-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };
        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
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

        // Cleanup CoreDB resource
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_delete_namespace() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let _context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-delete-namespace-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };
        let replicas = 1;

        let ns_api: Api<Namespace> = Api::all(client.clone());

        // Create coredb
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": "CoreDB",
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "aggs_for_vecs",
                        "version": "1.3.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "aggs_for_vecs",
                        "description": "aggs_for_vecs extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.3.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        coredbs.patch(name, &params, &patch).await.unwrap();

        // Assert coredb is running
        let pod_name = format!("{}-1", name);
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;

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
        let namespace_clone = namespace.clone();
        tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_NS_DELETED),
            async move {
                loop {
                    let get_ns = ns_api.get_opt(&namespace_clone).await.unwrap();
                    if get_ns.is_none() {
                        break;
                    }
                }
            },
        )
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
                    "serviceType": "ClusterIP"
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

    #[tokio::test]
    #[ignore]
    async fn functional_test_ha_basic_cnpg() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-ha-basic-cnpg-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 2;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
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
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pod_name_primary = format!("{}-1", name);
        let pod_name_secondary = format!("{}-2", name);

        pod_ready_and_running(pods.clone(), pod_name_primary.clone()).await;
        pod_ready_and_running(pods.clone(), pod_name_secondary.clone()).await;

        // Assert that we can query the database with \dt;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;
        assert!(result.stdout.clone().unwrap().contains("plpgsql"));

        // Assert that both pods are replicating successfully
        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "SELECT state FROM pg_stat_replication".to_string(),
        )
        .await;
        assert!(result.stdout.clone().unwrap().contains("streaming"));

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_ha_upgrade_cnpg() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-ha-upgrade-cnpg-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
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
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pod_name_primary = format!("{}-1", name);
        pod_ready_and_running(pods.clone(), pod_name_primary.clone()).await;

        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Assert that we can query the database with \dx;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;
        assert!(result.stdout.clone().unwrap().contains("plpgsql"));

        // Now upgrade the single instance to be HA
        let replicas = 2;
        // Generate HA CoreDB resource
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
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for new CNPG secondary Pod to be created and running
        let pod_name_secondary = format!("{}-2", name);
        pod_ready_and_running(pods.clone(), pod_name_secondary.clone()).await;

        // Assert that we can query the database again now that HA is enabled with \dx;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;
        assert!(result.stdout.clone().unwrap().contains("plpgsql"));

        // Assert that both pods are replicating successfully
        let mut retries = 0;
        loop {
            let result = psql_with_retry(
                context.clone(),
                coredb_resource.clone(),
                "SELECT state FROM pg_stat_replication".to_string(),
            )
            .await;

            if result.stdout.is_some() && result.stdout.clone().unwrap().contains("streaming") {
                println!("Replication is streaming.");
                assert!(result.stdout.clone().unwrap().contains("streaming"));
                break;
            } else if retries >= 10 {
                panic!("Replication is not streaming after 10 retries");
            } else {
                retries += 1;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        // Remove HA CoreDB resource
        let coredb_json_patch = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": 1,
            }
        });
        println!("Disabling HA by setting replicas to 1");
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json_patch);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for secondary CNPG pod to be deleted
        println!("Waiting for Pod {} to be deleted", pod_name_secondary);
        let _assert_secondary_deleted = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_POD_DELETED),
            await_condition(
                pods.clone(),
                &pod_name_secondary,
                conditions::is_deleted(""),
            ),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Pod {} was not deleted after waiting {} seconds",
                pod_name_secondary, TIMEOUT_SECONDS_POD_DELETED
            )
        });

        // Query the database again to ensure that pg_replication_slots is empty
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT count(*) from pg_replication_slots".to_string(),
            "0".to_string(),
            false,
        )
        .await;

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_shared_preload_libraries() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-requires-load-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
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
                        "name": "pg_cron",
                        "description": "cron",
                        "locations": [{
                            "enabled": true,
                            "version": "1.6.2",
                            "database": "postgres",
                        }],
                    },
                    {
                        "name": "citus",
                        "description": "citus",
                        "locations": [{
                            "enabled": true,
                            "version": "12.0.1",
                            "database": "postgres",
                        }],
                }],
                "trunk_installs": [{
                        "name": "pg_cron",
                        "version": "1.6.2",
                },
                {
                        "name": "citus",
                        "version": "12.0.1",
                }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        let pod_name = format!("{}-1", name);
        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "\\dx".to_string(),
            "pg_cron".to_string(),
            false,
        )
        .await;

        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "\\dx".to_string(),
            "citus".to_string(),
            false,
        )
        .await;

        let coredb_resource = coredbs.get(name).await.unwrap();
        let mut found_citus = false;
        let mut found_cron = false;
        for extension in coredb_resource.status.unwrap().extensions.unwrap() {
            for location in extension.locations {
                if extension.name == "citus" && location.enabled.unwrap() {
                    found_citus = true;
                    assert!(location.database == "postgres");
                    assert!(location.schema.clone().unwrap() == "pg_catalog");
                }
                if extension.name == "pg_cron" && location.enabled.unwrap() {
                    found_cron = true;
                    assert!(location.database == "postgres");
                    assert!(location.schema.unwrap() == "pg_catalog");
                }
            }
        }
        assert!(found_citus);
        assert!(found_cron);

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_ext_with_load() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-ext-with-load-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
        // Install extensions, but don't enable them
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name,
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [{
                        "name": "auto_explain",
                        "version": "15.3.0",
                },
                {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                },
                {
                        "name": "auth_delay",
                        "version": "15.3.0",
                },
                {
                        "name": "adminpack",
                        "version": "2.1.0",
                }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        //TODO(ianstanton) wait for status.extensions to be populated
        tokio::time::sleep(Duration::from_secs(20)).await;

        // Check status.extensions.locations is not empty
        let coredb_resource = coredbs.get(name).await.unwrap();
        for extension in coredb_resource.status.unwrap().extensions.unwrap() {
            assert!(!extension.locations.is_empty());
        }

        // Update CoreDB resource to enable extensions
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name,
            },
            "spec": {
                "replicas": replicas,
                "extensions": [{
                        "name": "auto_explain",
                        "description": "",
                        "locations": [{
                            "enabled": true,
                            "version": "15.3.0",
                            "database": "postgres",
                        }],
                    },
                    {
                        "name": "pg_stat_statements",
                        "description": "",
                        "locations": [{
                            "enabled": true,
                            "version": "1.10",
                            "database": "postgres",
                        }],
                    },
                    {
                        "name": "adminpack",
                        "description": "",
                        "locations": [{
                            "enabled": true,
                            "version": "2.1",
                            "database": "postgres",
                        }],
                    },
                    {
                        "name": "plperl",
                        "description": "",
                        "locations": [{
                            "enabled": true,
                            "version": "1.0",
                            "database": "postgres",
                        }],
                    },
                    {
                        "name": "auth_delay",
                        "description": "",
                        "locations": [{
                            "enabled": true,
                            "version": "15.3.0",
                            "database": "postgres",
                        }],
                }],
                "trunk_installs": [{
                        "name": "auto_explain",
                        "version": "15.3.0",
                },
                {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                },
                {
                        "name": "auth_delay",
                        "version": "15.3.0",
                },
                {
                        "name": "adminpack",
                        "version": "2.1.0",
                }]
            }
        });

        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be updated and ready
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // Assert psql show shared_preload_libraries contains auto_explain and auth_delay
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SHOW shared_preload_libraries".to_string(),
            "auto_explain".to_string(),
            false,
        )
        .await;

        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SHOW shared_preload_libraries".to_string(),
            "auth_delay".to_string(),
            false,
        )
        .await;

        // Assert psql show pg_available_extensions contains pg_stat_statements
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT * FROM pg_available_extensions".to_string(),
            "pg_stat_statements".to_string(),
            false,
        )
        .await;

        // Assert psql show pg_available_extensions does not contain auto_explain
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT * FROM pg_available_extensions".to_string(),
            "auto_explain".to_string(),
            true,
        )
        .await;

        // Assert psql show pg_available_extensions does not contain auth_delay
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT * FROM pg_available_extensions".to_string(),
            "auth_delay".to_string(),
            true,
        )
        .await;

        // Check auto_explain, auth_delay and pg_stat_statements are enabled in extension status.
        wait_for_extension_status_enabled(&coredbs, name, "auto_explain", false)
            .await
            .expect("Error: auto_explain was not enabled in status");

        wait_for_extension_status_enabled(&coredbs, name, "auth_delay", false)
            .await
            .expect("Error: auth_delay was not enabled in status");

        wait_for_extension_status_enabled(&coredbs, name, "pg_stat_statements", false)
            .await
            .expect("Error: pg_stat_statements was not enabled in status");

        // Disable auto_explain and auth_delay
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name,
            },
            "spec": {
                "replicas": replicas,
                "extensions": [{
                        "name": "auto_explain",
                        "description": "",
                        "locations": [{
                            "enabled": false,
                            "version": "15.3.0",
                            "database": "postgres",
                        }],
                    },
                    {
                        "name": "pg_stat_statements",
                        "description": "",
                        "locations": [{
                            "enabled": true,
                            "version": "1.10.0",
                            "database": "postgres",
                        }],
                    },
                    {
                        "name": "auth_delay",
                        "description": "",
                        "locations": [{
                            "enabled": false,
                            "version": "15.3.0",
                            "database": "postgres",
                        }],
                }],
                "trunk_installs": [{
                        "name": "auto_explain",
                        "version": "15.3.0",
                },
                {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                },
                {
                        "name": "auth_delay",
                        "version": "15.3.0",
                }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be updated and ready
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // Assert psql show shared_preload_libraries does not contain auto_explain and auth_delay
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SHOW shared_preload_libraries".to_string(),
            "auto_explain".to_string(),
            true,
        )
        .await;

        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SHOW shared_preload_libraries".to_string(),
            "auth_delay".to_string(),
            true,
        )
        .await;

        // Assert psql show pg_available_extensions contains pg_stat_statements
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT * FROM pg_available_extensions".to_string(),
            "pg_stat_statements".to_string(),
            false,
        )
        .await;

        // Assert psql show pg_available_extensions does not contain auto_explain
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT * FROM pg_available_extensions".to_string(),
            "auto_explain".to_string(),
            true,
        )
        .await;

        // Assert psql show pg_available_extensions does not contain auth_delay
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT * FROM pg_available_extensions".to_string(),
            "auth_delay".to_string(),
            true,
        )
        .await;

        // Check auto_explain and auth_delay are disabled in extension status. Check pg_stat_statements is still enabled.
        wait_for_extension_status_enabled(&coredbs, name, "auto_explain", true)
            .await
            .expect("Error: auto_explain was not disabled in status");

        wait_for_extension_status_enabled(&coredbs, name, "auth_delay", true)
            .await
            .expect("Error: auth_delay was not disabled in status");

        wait_for_extension_status_enabled(&coredbs, name, "pg_stat_statements", false)
            .await
            .expect("Error: pg_stat_statements was not enabled in status");

        // Cleanup
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_ha_two_replicas() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-ha-two-replicas-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 2;

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
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
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Wait for CNPG Cluster to be created by looping over replicas until
        // they are in a running state
        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        // Assert that we can query the database with \dx;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;
        assert!(result.stdout.clone().unwrap().contains("plpgsql"));

        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        // Assert that both pods are replicating successfully
        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "SELECT state FROM pg_stat_replication".to_string(),
        )
        .await;
        assert!(result.stdout.clone().unwrap().contains("streaming"));

        // Add in an extension and lets make sure it's installed on all pods
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "aggs_for_vecs",
                        "version": "1.3.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "aggs_for_vecs",
                        "description": "aggs_for_vecs extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.3.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait until the extension is installed
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "aggs_for_vecs".to_string(),
            true,
        )
        .await;

        // Assert that the extensions are installed on both replicas
        let retrieved_pods_result = coredb_resource.pods_by_cluster(client.clone()).await;

        let retrieved_pods = match retrieved_pods_result {
            Ok(pods_list) => pods_list,
            Err(e) => {
                panic!("Failed to retrieve pods: {:?}", e);
            }
        };
        for pod in &retrieved_pods {
            let cmd = vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "ls /var/lib/postgresql/data/tembo/extension/aggs_for_vecs.control".to_owned(),
            ];
            let pod_name = pod.metadata.name.clone().expect("Pod should have a name");
            let result = run_command_in_container(
                pods.clone(),
                pod_name,
                cmd.clone(),
                Some("postgres".to_string()),
            )
            .await;
            assert!(result.contains("aggs_for_vecs.control"));
        }

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_ha_verify_extensions_ha_later() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-ha-verify-extension-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
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
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Cluster to be created by looping over replicas until
        // they are in a running state
        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        let _result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "plpgsql".to_string(),
            false,
        )
        .await;

        // Add in an extension and lets make sure it's installed on all pods
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "aggs_for_vecs",
                        "version": "1.3.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "aggs_for_vecs",
                        "description": "aggs_for_vecs extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.3.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait until the extension is installed
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            LIST_EXTENSIONS_QUERY.to_string(),
            "aggs_for_vecs".to_string(),
            false,
        )
        .await;

        // Assert that the extensions are installed on both replicas
        let retrieved_pods_result = coredb_resource.pods_by_cluster(client.clone()).await;

        let retrieved_pods = match retrieved_pods_result {
            Ok(pods_list) => pods_list,
            Err(e) => {
                panic!("Failed to retrieve pods: {:?}", e);
            }
        };
        for pod in &retrieved_pods {
            let cmd = vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "ls /var/lib/postgresql/data/tembo/extension/aggs_for_vecs.control".to_owned(),
            ];
            let pod_name = pod.metadata.name.clone().expect("Pod should have a name");
            let result = run_command_in_container(
                pods.clone(),
                pod_name,
                cmd.clone(),
                Some("postgres".to_string()),
            )
            .await;
            assert!(result.contains("aggs_for_vecs.control"));
        }

        // Now lets make the instance HA and ensure that all extensions are present on both
        // replicas
        let replicas = 2;
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "aggs_for_vecs",
                        "version": "1.3.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "aggs_for_vecs",
                        "description": "aggs_for_vecs extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.3.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    }]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for new replicas to be spun up before checking for extensions
        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        // Wait until the extension is installed
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "aggs_for_vecs".to_string(),
            true,
        )
        .await;

        // Assert that the extensions are installed on both replicas
        let retrieved_pods_result = coredb_resource.pods_by_cluster(client.clone()).await;

        let retrieved_pods = match retrieved_pods_result {
            Ok(pods_list) => pods_list,
            Err(e) => {
                panic!("Failed to retrieve pods: {:?}", e);
            }
        };
        for pod in &retrieved_pods {
            let cmd = vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "ls /var/lib/postgresql/data/tembo/extension/aggs_for_vecs.control".to_owned(),
            ];
            let pod_name = pod.metadata.name.clone().expect("Pod should have a name");
            let result = run_command_in_container(
                pods.clone(),
                pod_name,
                cmd.clone(),
                Some("postgres".to_string()),
            )
            .await;
            assert!(result.contains("aggs_for_vecs.control"));
        }

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_ha_shared_preload_libraries() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-ha-requires-load-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "pg_partman",
                        "version": "4.7.3",
                    },
                    {
                        "name": "pgmq",
                        "version": "0.10.0",
                    },
                    {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "pg_partman",
                        "description": "pg_partman extension",
                        "locations": [{
                            "enabled": false,
                            "version": "4.7.3",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pgmq",
                        "description": "pgmq extension",
                        "locations": [{
                            "enabled": false,
                            "version": "0.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pg_stat_statements",
                        "description": "pg_stat_statements extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Cluster to be created by looping over replicas until
        // they are in a running state
        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        // Assert that we can query the database with \dx;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;

        let stdout = match result.stdout {
            Some(output) => output,
            None => panic!("stdout is None"),
        };

        assert!(stdout.contains("plpgsql"));

        // Wait until the extension is installed
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "pgmq".to_string(),
            true,
        )
        .await;

        // Assert that the extensions are installed on both replicas
        let retrieved_pods_result = coredb_resource.pods_by_cluster(client.clone()).await;

        let retrieved_pods = match retrieved_pods_result {
            Ok(pods_list) => pods_list,
            Err(e) => {
                panic!("Failed to retrieve pods: {:?}", e);
            }
        };

        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        for pod in &retrieved_pods {
            let cmd = vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "ls /var/lib/postgresql/data/tembo/extension/pgmq.control".to_owned(),
            ];
            let pod_name = pod.metadata.name.clone().expect("Pod should have a name");
            let mut retries = 0;
            loop {
                let result = run_command_in_container(
                    pods.clone(),
                    pod_name.clone(),
                    cmd.clone(),
                    Some("postgres".to_string()),
                )
                .await;
                if !result.is_empty() || retries >= 10 {
                    assert!(result.contains("pgmq.control"));
                    break;
                } else {
                    retries += 1;
                    println!(
                        "Waiting for pgmq.control to be present, retry: {}/10",
                        retries
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        // Now lets make the instance HA and ensure that all extensions are present on both
        // replicas
        let replicas = 2;
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "pg_partman",
                        "version": "4.7.3",
                    },
                    {
                        "name": "pgmq",
                        "version": "0.10.0",
                    },
                    {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "pg_partman",
                        "description": "pg_partman extension",
                        "locations": [{
                            "enabled": false,
                            "version": "4.7.3",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pgmq",
                        "description": "pgmq extension",
                        "locations": [{
                            "enabled": false,
                            "version": "0.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pg_stat_statements",
                        "description": "pg_stat_statements extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for new replicas to be spun up before checking for extensions
        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        // Wait until the extension is installed
        wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "select extname from pg_catalog.pg_extension;".to_string(),
            "pgmq".to_string(),
            true,
        )
        .await;

        // Assert that the extensions are installed on both replicas
        let retrieved_pods_result = coredb_resource.pods_by_cluster(client.clone()).await;

        let retrieved_pods = match retrieved_pods_result {
            Ok(pods_list) => pods_list,
            Err(e) => {
                panic!("Failed to retrieve pods: {:?}", e);
            }
        };
        for pod in &retrieved_pods {
            let cmd = vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "ls /var/lib/postgresql/data/tembo/extension/pgmq.control".to_owned(),
            ];
            let pod_name = pod.metadata.name.clone().expect("Pod should have a name");
            let result = run_command_in_container(
                pods.clone(),
                pod_name,
                cmd.clone(),
                Some("postgres".to_string()),
            )
            .await;
            assert!(result.contains("pgmq.control"));
        }

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_app_service() {
        // Initialize the Kubernetes client
        let client = kube_client().await;

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let cdb_name = &format!("test-app-service-{}", suffix);
        let namespace = match create_namespace(client.clone(), cdb_name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", cdb_name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // generate an instance w/ 2 appServices
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": cdb_name
            },
            "spec": {
                "extensions": [
                    {
                        "name": "pg_graphql",
                        "locations": [
                            {
                                "database": "postgres",
                                "enabled": true,
                                "version": "1.4.1"
                            }
                        ]
                    }
                ],
                "trunk_installs": [
                    {
                        "name": "pg_graphql",
                        "version": "1.4.1"
                    }
                ],
                "appServices": [
                    {
                        "name": "postgrest",
                        "image": "postgrest/postgrest:v10.0.0",
                        "env": [
                            {
                                "name": "PGRST_DB_URI",
                                "valueFromPlatform": "ReadWriteConnection"
                            },
                            {
                                "name": "PGRST_DB_SCHEMA",
                                "value": "public"
                            },
                            {
                                "name": "PGRST_DB_ANON_ROLE",
                                "value": "postgres"
                            }
                        ],
                        "routing": [
                            {
                                "port": 3000,
                                "ingressPath": "/",
                                "ingressType": "http"
                            }
                        ],
                        "resources": {
                            "requests": {
                                "cpu": "100m",
                                "memory": "256Mi"
                            },
                            "limits": {
                                "cpu": "100m",
                                "memory": "256Mi"
                            }
                        }
                    },
                    {
                        "name": "test-app-1",
                        "image": "crccheck/hello-world:latest",
                        "resources": {
                            "requests": {
                                "cpu": "50m",
                                "memory": "128Mi"
                            },
                            "limits": {
                                "cpu": "50m",
                                "memory": "128Mi"
                            }
                        }
                    },
                    {
                        "name": "ferretdb",
                        "image": "ghcr.io/ferretdb/ferretdb",
                        "routing": [
                            {
                                "port": 27018,
                                "ingressPath": "/ferretdb/v1",
                                "entryPoints": [
                                    "ferretdb"
                                ],
                                "ingressType": "tcp"
                            }
                        ],
                        "env": [
                            {
                                "name": "FERRETDB_POSTGRESQL_URL",
                                "valueFromPlatform": "ReadWriteConnection"
                            },
                            {
                                "name": "FERRETDB_LOG_LEVEL",
                                "value": "debug"
                            },
                            {
                                "name": "FERRETDB_STATE_DIR",
                                "value": "-"
                            },
                            {
                                "name": "FERRETDB_LISTEN_TLS_CERT_FILE",
                                "value": "/certs/tls.crt"
                            },
                        ],
                        "storage": {
                            "volumes": [
                                {
                                    "name": "ferretdb-data",
                                    "ephemeral": {
                                        "volumeClaimTemplate": {
                                            "spec": {
                                                "accessModes": [
                                                    "ReadWriteOnce"
                                                ],
                                                "resources": {
                                                    "requests": {
                                                        "storage": "1Gi"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            ],
                            "volumeMounts": [
                                {
                                    "name": "ferretdb-data",
                                    "mountPath": "/state"
                                }
                            ]
                        },
                    }
                ],
                "postgresExporterEnabled": false
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(cdb_name, &params, &patch).await.unwrap();

        // assert we created three Deployments, with the names we provided
        let deployment_items: Vec<Deployment> =
            list_resources(client.clone(), cdb_name, &namespace, 3)
                .await
                .unwrap();
        // three AppService deployments. the postgres exporter is disabled
        assert!(deployment_items.len() == 3);

        let service_items: Vec<Service> = list_resources(client.clone(), cdb_name, &namespace, 2)
            .await
            .unwrap();
        // two AppService Services, because two of the AppServices expose ports
        assert!(service_items.len() == 2);

        let app_0 = deployment_items[0].clone();
        let app_1 = deployment_items[1].clone();
        let app_2 = deployment_items[2].clone();
        assert_eq!(app_0.metadata.name.unwrap(), format!("{cdb_name}-ferretdb"));
        assert_eq!(
            app_1.metadata.name.unwrap(),
            format!("{cdb_name}-postgrest")
        );
        assert_eq!(
            app_2.metadata.name.unwrap(),
            format!("{cdb_name}-test-app-1")
        );

        let selector_map = app_0
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.as_ref())
            .expect("Deployment should have a selector");
        let selector = selector_map
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",");
        let lp = ListParams::default().labels(&selector);
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        let pod_list = pods.list(&lp).await.unwrap();
        assert_eq!(pod_list.items.len(), 1);
        let app_0_pod = pod_list.items[0].clone();
        let app_0_container = app_0_pod.spec.unwrap().containers[0].clone();

        // Assert app_0 volume mounts include ferretdb-data and tembo-certs
        let volume_mounts = app_0_container.volume_mounts.unwrap();
        let mut found_ferretdb_data = false;
        let mut found_tembo_certs = false;
        for mount in volume_mounts {
            if mount.mount_path == "/state" {
                found_ferretdb_data = true;
            }
            if mount.mount_path == "/tembo/certs" {
                found_tembo_certs = true;
            }
        }
        assert!(found_ferretdb_data);
        assert!(found_tembo_certs);

        // Assert resources in first appService
        // select the pod
        let selector_map = app_1
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.as_ref())
            .expect("Deployment should have a selector");
        let selector = selector_map
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",");
        let lp = ListParams::default().labels(&selector);
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        // Fetch and print all the pods matching the label selector
        let pod_list = pods.list(&lp).await.unwrap();
        assert_eq!(pod_list.items.len(), 1);
        let app_1_pod = pod_list.items[0].clone();
        let app_1_container = app_1_pod.spec.unwrap().containers[0].clone();

        let expected: ResourceRequirements = serde_json::from_value(serde_json::json!({
            "requests": {
                "cpu": "100m",
                "memory": "256Mi"
            },
            "limits": {
                "cpu": "100m",
                "memory": "256Mi"
            }
        }))
        .unwrap();
        let app_1_resources = app_1_container.resources.unwrap();
        assert_eq!(app_1_resources, expected);

        // Assert /tembo/certs is included in volume mounts
        let volume_mounts = app_1_container.volume_mounts.unwrap();
        let mut found = false;
        for mount in volume_mounts {
            if mount.mount_path == "/tembo/certs" {
                found = true;
            }
        }
        assert!(found);

        let ingresses: Result<Vec<IngressRoute>, errors::OperatorError> =
            list_resources(client.clone(), cdb_name, &namespace, 1).await;
        let ingress = ingresses.unwrap();
        assert_eq!(ingress.len(), 1);
        let ingress_route = ingress[0].clone();
        let routes = ingress_route.spec.clone().routes.clone();
        assert_eq!(routes.len(), 1);
        let route = routes[0].clone();
        assert_eq!(
            route.r#match,
            format!("Host(`{}.localhost`) && PathPrefix(`/`)", cdb_name)
        );

        // Assert entry_points includes only websecure
        assert_eq!(
            ingress_route.spec.entry_points,
            Some(vec!["websecure".to_string()])
        );

        // Check for IngressRouteTCP
        let ing_name = format!("{cdb_name}-ferretdb");
        let ingresses_tcp: Result<Vec<IngressRouteTCP>, errors::OperatorError> =
            list_resources(client.clone(), &ing_name, &namespace, 1).await;
        let ingress_tcp = ingresses_tcp.unwrap();
        assert_eq!(ingress_tcp.len(), 1);
        let ingress_route_tcp = ingress_tcp[0].clone();
        let routes_tcp = ingress_route_tcp.spec.clone().routes.clone();
        assert_eq!(routes.len(), 1);
        let route_tcp = routes_tcp[0].clone();
        assert_eq!(
            route_tcp.r#match,
            format!("HostSNI(`{}.localhost`)", cdb_name)
        );

        // Assert entry_points includes only ferretdb
        assert_eq!(
            ingress_route_tcp.spec.entry_points,
            Some(vec!["ferretdb".to_string()])
        );

        let services = routes[0].services.clone().unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, format!("{}-postgrest", cdb_name));
        assert_eq!(services[0].port.clone().unwrap(), IntOrString::Int(3000));

        // Assert resources in second AppService
        let selector_map = app_2
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.as_ref())
            .expect("Deployment should have a selector");
        let selector = selector_map
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",");
        let lp = ListParams::default().labels(&selector);
        let pod_list = pods.list(&lp).await.unwrap();
        assert_eq!(pod_list.items.len(), 1);
        let app_2_pod = pod_list.items[0].clone();
        let app_2_container = app_2_pod.spec.unwrap().containers[0].clone();

        let expected: ResourceRequirements = serde_json::from_value(serde_json::json!({
            "requests": {
                "cpu": "50m",
                "memory": "128Mi"
            },
            "limits": {
                "cpu": "50m",
                "memory": "128Mi"
            }
        }))
        .unwrap();
        let app_2_resources = app_2_container.resources.unwrap();
        assert_eq!(app_2_resources, expected);

        // Assert /tembo/certs is included in volume mounts
        let volume_mounts = app_2_container.volume_mounts.unwrap();
        let mut found = false;
        for mount in volume_mounts {
            if mount.mount_path == "/tembo/certs" {
                found = true;
            }
        }
        assert!(found);

        // Delete the one without a service, but leave the postgrest appService
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": cdb_name
            },
            "spec": {
                "appServices": [
                    {
                        "name": "postgrest",
                        "image": "postgrest/postgrest:v10.0.0",
                        "env": [
                            {
                                "name": "PGRST_DB_URI",
                                "valueFromPlatform": "ReadWriteConnection"
                            },
                            {
                                "name": "PGRST_DB_SCHEMA",
                                "value": "public"
                            },
                            {
                                "name": "PGRST_DB_ANON_ROLE",
                                "value": "postgres"
                            }
                        ],
                        "routing": [
                            {
                                "port": 3000,
                                "ingressPath": "/"
                            }
                        ],
                    }
                ],
                "postgresExporterEnabled": false
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        coredbs.patch(cdb_name, &params, &patch).await.unwrap();

        let deployment_items: Vec<Deployment> =
            list_resources(client.clone(), cdb_name, &namespace, 1)
                .await
                .unwrap();
        assert!(deployment_items.len() == 1);
        let app_0 = deployment_items[0].clone();
        assert_eq!(
            app_0.metadata.name.unwrap(),
            format!("{cdb_name}-postgrest")
        );

        // should still be just one Service
        let service_items: Vec<Service> = list_resources(client.clone(), cdb_name, &namespace, 1)
            .await
            .unwrap();
        // One appService Services
        assert!(service_items.len() == 1);

        // send a request to postgres
        #[derive(Debug, Deserialize)]
        struct ApiResponse {
            info: ApiInfo,
        }

        #[derive(Debug, Deserialize)]
        struct ApiInfo {
            title: String,
        }
        let postgres_url = format!("https://{}.localhost:8443/", cdb_name);
        // with no headers, request will succeed against postgREST
        let response = http_get_with_retry(&postgres_url, None, 100, 5)
            .await
            .unwrap();
        let body: ApiResponse = response.json().await.unwrap();
        assert_eq!(body.info.title, "PostgREST API");

        // add an auth header and request will fail (have not configured server side JWT)
        let headers: BTreeMap<String, String> =
            [(String::from("Authrization"), String::from("Bearer SomeKey"))]
                .iter()
                .cloned()
                .collect();
        let response = http_get_with_retry(&postgres_url, Some(headers.clone()), 1, 0).await;
        assert!(response.is_err());

        // patch the postgREST/graphql appService with required middlewares for header and prefix
        // service it at /rest, but route traffic to container at /
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": cdb_name
            },
            "spec": {
                "extensions": [
                    {
                        "name": "pg_graphql",
                        "locations": [
                            {
                                "database": "postgres",
                                "enabled": true,
                                "version": "1.4.1"
                            }
                        ]
                    }
                ],
                "trunk_installs": [
                    {
                        "name": "pg_graphql",
                        "version": "1.4.1"
                    }
                ],
                "appServices": [
                    {
                        "name": "postgrest",
                        "image": "postgrest/postgrest:v10.0.0",
                        "env": [
                            {
                                "name": "PGRST_DB_URI",
                                "valueFromPlatform": "ReadWriteConnection"
                            },
                            {
                                "name": "PGRST_DB_SCHEMA",
                                "value": "public, graphql"
                            },
                            {
                                "name": "PGRST_DB_ANON_ROLE",
                                "value": "postgres"
                            }
                        ],
                        "middlewares": [
                            {
                                "customRequestHeaders": {
                                    "name": "my-header",
                                    "config": {
                                        "Authorization": "",
                                        "Content-Profile": "graphql",
                                        "Accept-Profile": "graphql"
                                    }
                                }
                            },
                            {
                                "stripPrefix": {
                                    "name": "strip-prefix",
                                    "config": [
                                        "/rest"
                                    ]
                                }
                            },
                            {
                                "replacePathRegex": {
                                    "name": "map-gql",
                                    "config":
                                        {
                                            "regex": "/graphql",
                                            "replacement": "/rpc/resolve"
                                        }
                                },
                            }
                        ],
                        "routing": [
                            {
                                "port": 3000,
                                "ingressPath": "/rest",
                                "middlewares": [
                                    "my-header",
                                    "strip-prefix"
                                ]
                            },
                            {
                                "port": 3000,
                                "ingressPath": "/graphql",
                                "middlewares": [
                                    "my-header",
                                    "map-gql"
                                ]
                            },
                        ],
                    }
                ],
                "postgresExporterEnabled": false
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let cdb = coredbs.patch(cdb_name, &params, &patch).await.unwrap();
        // same request with auth header will now succeed
        // add some retries to give change a chance to apply
        let response = http_get_with_retry(&postgres_url, Some(headers.clone()), 30, 5)
            .await
            .unwrap();
        let body: ApiResponse = response.json().await.unwrap();
        assert_eq!(body.info.title, "PostgREST API");

        let trigger = "
        CREATE OR REPLACE FUNCTION pgrst_watch() RETURNS event_trigger
  LANGUAGE plpgsql
  AS $$
BEGIN
  NOTIFY pgrst, 'reload schema';
END;
$$;

CREATE EVENT TRIGGER pgrst_watch
  ON ddl_command_end
  EXECUTE PROCEDURE pgrst_watch();
";
        //
        let state = State::default();
        let context = state.create_context(client.clone());
        // hard sleep to give operator time to apply change
        // tokio::time::sleep(Duration::from_secs(5)).await;
        let result = psql_with_retry(context.clone(), cdb.clone(), trigger.to_string()).await;
        println!("result: {:#?}", result);
        assert!(result.success);
        // create a table for gql to inflect
        let _result = psql_with_retry(
            context.clone(),
            cdb.clone(),
            "create table book (id serial primary key, name text);".to_string(),
        )
        .await;

        // send a request to graphql route
        let gql_uri = format!("{}graphql?query=%7B%20bookCollection%20%7B%20edges%20%7B%20node%20%7B%20id%20%7D%20%7D%20%7D%20%7D", postgres_url);
        // panics if its a non-200 response
        let _response = http_get_with_retry(&gql_uri, Some(headers), 10, 5)
            .await
            .unwrap();

        // Delete all of them
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": cdb_name
            },
            "spec": {
                "postgresExporterEnabled": false
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        coredbs.patch(cdb_name, &params, &patch).await.unwrap();
        let deployment_items: Vec<Deployment> =
            list_resources(client.clone(), cdb_name, &namespace, 0)
                .await
                .unwrap();
        assert!(deployment_items.is_empty());

        let service_items: Vec<Service> = list_resources(client.clone(), cdb_name, &namespace, 0)
            .await
            .unwrap();
        assert!(service_items.is_empty());
        // should be no Services

        // ingress must be gone
        let ingresses: Vec<IngressRoute> = list_resources(client.clone(), cdb_name, &namespace, 0)
            .await
            .unwrap();
        assert_eq!(ingresses.len(), 0);

        // Assert IngressRouteTCP is gone
        let ingresses_tcp: Vec<IngressRouteTCP> =
            list_resources(client.clone(), cdb_name, &namespace, 0)
                .await
                .unwrap();
        assert_eq!(ingresses_tcp.len(), 0);

        // CLEANUP TEST
        // Cleanup CoreDB
        coredbs.delete(cdb_name, &Default::default()).await.unwrap();
        println!("Waiting for CoreDB to be deleted: {}", &cdb_name);
        let _assert_coredb_deleted = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_COREDB_DELETED),
            await_condition(coredbs.clone(), cdb_name, conditions::is_deleted("")),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "CoreDB {} was not deleted after waiting {} seconds",
                cdb_name, TIMEOUT_SECONDS_COREDB_DELETED
            )
        });
        println!("CoreDB resource deleted {}", cdb_name);

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn restarts_postgres_correctly() {
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

        // Initialize tracing
        tracing_subscriber::fmt().init();

        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        let name = {
            let mut rng = rand::thread_rng();
            let suffix = rng.gen_range(0..100000);

            format!("test-restart-postgres-{}", suffix)
        };

        let namespace = create_namespace(client.clone(), &name).await.unwrap();

        // Apply a basic configuration of CoreDB
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": "CoreDB",
            "metadata": {
                "name": name,
            },
            "spec": {
                "replicas": 1,
            }
        });

        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(&name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        {
            let pod_name = format!("{}-1", name);

            pod_ready_and_running(pods.clone(), pod_name.clone()).await;
            wait_til_status_is_filled(&coredbs, &name).await;
        }

        // Ensure status.running is true
        assert!(status_running(&coredbs, &name).await);
        let initial_start_time = get_pg_start_time(&coredbs, &name, context.clone()).await;

        // Initialize uninterruptible query
        let _ = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "\
                CREATE EXTENSION IF NOT EXISTS plpython3u;
                CREATE FUNCTION slow_fibonacci (n integer)
                  RETURNS integer
                AS $$
                  def recur_fibo(n):
                    if n <= 1:
                      return n
                    else:
                      return(recur_fibo(n-1) + recur_fibo(n-2))
                  return recur_fibo(n)
                $$ LANGUAGE plpython3u;
            "
            .to_string(),
        )
        .await;

        let _stuck_query = coredb_resource.psql(
            "SELECT slow_fibonacci(100);".to_string(),
            "postgres".to_string(),
            context.clone(),
        );

        // Apply the annotation to restart Postgres
        {
            let restart = Utc::now()
                .to_rfc3339_opts(SecondsFormat::Secs, true)
                .to_string();

            let patch_json = serde_json::json!({
                "metadata": {
                    "annotations": {
                        "kubectl.kubernetes.io/restartedAt": restart
                    }
                }
            });

            let patch = Patch::Merge(&patch_json);
            coredbs
                .patch(&name, &PatchParams::default(), &patch)
                .await
                .unwrap();
        }

        // Ensure that eventually `status.running` becomes false to reflect
        // that Postgres is down
        {
            match wait_until_status_not_running(&coredbs, &name).await {
                Ok(_) => println!("status.running is now false!"),
                Err(e) => panic!("status.running should've become false after restart: {}", e),
            }
        }

        // Wait for Postgres to restart
        {
            let started = Utc::now();
            let max_wait_time =
                chrono::TimeDelta::try_seconds(TIMEOUT_SECONDS_POD_READY as _).unwrap();
            let mut running_became_true = false;
            while Utc::now().signed_duration_since(started) < max_wait_time {
                if status_running(&coredbs, &name).await.not() {
                    println!("status.running is still false. Retrying in 3 secs.");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                } else {
                    println!("status.running is now true once again!");

                    running_became_true = true;
                    break;
                }
            }

            assert!(
                running_became_true,
                "status.running should've become true once restarted"
            );

            let reboot_start_time = get_pg_start_time(&coredbs, &name, context).await;

            assert!(
                reboot_start_time > initial_start_time,
                "start time should've changed"
            );
        }

        // Perform cleanup
        {
            coredbs.delete(&name, &Default::default()).await.unwrap();
            println!("Waiting for CoreDB to be deleted: {name}");
            let _assert_coredb_deleted = tokio::time::timeout(
                Duration::from_secs(TIMEOUT_SECONDS_COREDB_DELETED),
                await_condition(coredbs.clone(), &name, conditions::is_deleted("")),
            )
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "CoreDB {} was not deleted after waiting {} seconds",
                    name, TIMEOUT_SECONDS_COREDB_DELETED
                )
            });
            println!("CoreDB resource deleted {name}");

            // Delete namespace
            let _ = delete_namespace(client.clone(), &namespace).await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_status_configs() {
        async fn runtime_cfg(coredbs: &Api<CoreDB>, name: &str) -> Option<Vec<PgConfig>> {
            let started_waiting = Utc::now();
            let max_wait_time = chrono::TimeDelta::try_seconds(45).unwrap();

            while Utc::now().signed_duration_since(started_waiting) <= max_wait_time {
                let coredb = coredbs.get(name).await.expect("spec not found");
                if coredb.status.is_some() {
                    return coredb.status.unwrap().runtime_config.clone();
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            panic!("Status was not populated fast enough");
        }

        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let _context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-status-configs-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "pg_partman",
                        "version": "4.7.3",
                    },
                    {
                        "name": "pgmq",
                        "version": "0.10.0",
                    },
                    {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "pg_partman",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "4.7.3",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "pgmq",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "0.10.0",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "pg_stat_statements",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "1.10.0",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    }
                ],
                "runtime_config": [
                    {
                        "name": "shared_preload_libraries",
                        "value": "pg_stat_statements,pg_partman_bgw"
                    }
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // Assert status contains configs
        let mut found_configs = false;
        let expected_config = ConfigValue::Multiple(BTreeSet::from_iter(vec![
            "pg_stat_statements".to_string(),
            "pg_partman_bgw".to_string(),
        ]));

        // Wait for status.runtime_config to contain expected_config
        while !found_configs {
            let runtime_config = runtime_cfg(&coredbs, name).await;
            if runtime_config.is_some() {
                let runtime_config = runtime_config.unwrap();
                for config in runtime_config {
                    if config.name == "shared_preload_libraries" && config.value == expected_config
                    {
                        found_configs = true;
                    }
                }
            }
            println!(
                "Waiting for runtime_config for {} to be populated with expected values",
                name
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        // Assert status.runtime_config length is greater than 350. It should be around 362, but
        // that will fluctuate between postgres versions. This is a sanity check to ensure that
        // the runtime_config is being populated with all config values.
        let runtime_cfg = runtime_cfg(&coredbs, name).await.unwrap();
        assert!(runtime_cfg.len() > 350);
        println!("Found {} runtime_config values", runtime_cfg.len());

        // Assert status.runtime_config
        assert!(found_configs);

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    async fn status_running(coredbs: &Api<CoreDB>, name: &str) -> bool {
        let max_retries = 20;
        let wait_duration = Duration::from_secs(6); // Adjust as needed

        for attempt in 1..=max_retries {
            let coredb = coredbs.get(name).await.expect("Failed to get CoreDB");

            if coredb.status.as_ref().map_or(false, |s| s.running) {
                println!("CoreDB {} is running", name);
                return true;
            } else {
                println!(
                    "Attempt {}/{}: CoreDB {} is not running yet",
                    attempt, max_retries, name
                );
            }
            tokio::time::sleep(wait_duration).await;
        }
        println!(
            "CoreDB {} did not become running after {} attempts",
            name, max_retries
        );
        false
    }

    async fn pooler_status_running(poolers: &Api<Pooler>, name: &str) -> bool {
        let max_retries = 20;
        let wait_duration = Duration::from_secs(6); // Adjust as needed

        for attempt in 1..=max_retries {
            match poolers.get(name).await {
                Ok(pooler) => {
                    let instances = pooler
                        .status
                        .as_ref()
                        .and_then(|s| s.instances)
                        .unwrap_or(0);
                    if instances == 1 {
                        println!("Pooler {} is running", name);
                        return true;
                    } else {
                        println!(
                            "Attempt {}/{}: Pooler {} is not running yet (instances: {})",
                            attempt, max_retries, name, instances
                        );
                    }
                }
                Err(e) => {
                    println!("Error getting Pooler {}: {:?}", name, e);
                    // Decide if you want to return false here or continue retrying
                }
            }
            tokio::time::sleep(wait_duration).await;
        }
        println!(
            "Pooler {} did not become running after {} attempts",
            name, max_retries
        );
        false
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_backup_and_restore() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-backup-restore-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        let backup_location = format!("s3://tembo-backup/{}", name);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
            "replicas": replicas,
                "backup": {
                    "destinationPath": backup_location,
                    "retentionPolicy": "30",
                    "schedule": "17 9 * * *",
                    "encryption": "",
                    "endpointURL": "http://minio.minio.svc.cluster.local:9000",
                    "s3Credentials": {
                        "accessKeyId": {
                        "name": "s3creds",
                        "key": "MINIO_ACCESS_KEY"
                        },
                        "secretAccessKey": {
                        "name": "s3creds",
                        "key": "MINIO_SECRET_KEY"
                        }
                    },
                    "volumeSnapshot": {
                        "enabled": false,
                    }
                },
                "trunk_installs": [
                    {
                        "name": "pg_partman",
                        "version": "4.7.3",
                    },
                    {
                        "name": "pgmq",
                        "version": "0.10.0",
                    },
                    {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "pg_partman",
                        "description": "pg_partman extension",
                        "locations": [{
                            "enabled": false,
                            "version": "4.7.3",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pgmq",
                        "description": "pgmq extension",
                        "locations": [{
                            "enabled": false,
                            "version": "0.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pg_stat_statements",
                        "description": "pg_stat_statements extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pod_name = format!("{}-1", name);
        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
        // Wait for CNPG Cluster to be created by looping over replicas until
        // they are in a running state
        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        // Assert that we can query the database with \dx;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;
        assert!(result.stdout.clone().unwrap().contains("plpgsql"));

        // Check to make sure the initial backup has run and its completed
        has_backup_completed(context.clone(), &namespace, name).await;

        // Create a table and insert some data
        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "CREATE TABLE test (id SERIAL PRIMARY KEY, name VARCHAR(255));".to_string(),
        )
        .await;
        assert!(result.stdout.clone().unwrap().contains("CREATE TABLE"));

        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "INSERT INTO test (name) VALUES ('test');".to_string(),
        )
        .await;
        assert!(result.stdout.clone().unwrap().contains("INSERT 0 1"));

        // Now take a new backup of the instance
        let backup: Api<Backup> = Api::namespaced(client.clone(), &namespace);
        let backup_name = format!("{}-backup", name);
        let backup_json = serde_json::json!({
            "apiVersion": "postgresql.cnpg.io/v1",
            "kind": "Backup",
            "metadata": {
                "name": backup_name,
                "labels": {
                    "cnpg.io/cluster": backup_name,
                    "cnpg.io/immediateBackup": "false",
                    "cnpg.io/scheduled-backup": name
                },
            },
            "spec": {
                "cluster": {
                    "name": name,
                }
            },
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&backup_json);
        let _backup_resource = backup.patch(&backup_name, &params, &patch).await.unwrap();

        // Assert that we can query the database with \dx;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;
        assert!(result.stdout.clone().unwrap().contains("plpgsql"));

        // Wait for backup to complete
        has_backup_completed(context.clone(), &namespace, &backup_name).await;

        // Check to make sure CoreDBStatus has last_archiver_status set to a DateTime<Utc>
        let cdbstatus = coredbs.get(name).await.unwrap();
        let last_archiver_state = cdbstatus
            .status
            .as_ref()
            .and_then(|stats| stats.last_archiver_status);
        assert!(last_archiver_state.is_some());

        // If the backup is complete, we can now restore to a new instance in a new namespace
        let suffix = rng.gen_range(0..100000);
        let restore_name = &format!("test-coredb-restore-{}", suffix);
        let restore_namespace = match create_namespace(client.clone(), restore_name).await {
            Ok(restore_namespace) => restore_namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let restore_pods: Api<Pod> = Api::namespaced(client.clone(), &restore_namespace);
        let restore_backup_location = format!("s3://tembo-backup/{}", restore_name);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", restore_name);
        let restore_coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &restore_namespace);
        // Generate basic CoreDB resource to start with
        let restore_coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": restore_name
            },
            "spec": {
                "replicas": replicas,
                "backup": {
                    "destinationPath": restore_backup_location,
                    "retentionPolicy": "30",
                    "schedule": "17 9 * * *",
                    "endpointURL": "http://minio.minio.svc.cluster.local:9000",
                    "encryption": "",
                    "s3Credentials": {
                        "accessKeyId": {
                            "name": "s3creds",
                            "key": "MINIO_ACCESS_KEY"
                        },
                        "secretAccessKey": {
                            "name": "s3creds",
                            "key": "MINIO_SECRET_KEY"
                        }
                    },
                    "volumeSnapshot": {
                        "enabled": false,
                    }
                },
                "restore": {
                    "serverName": name,
                    "endpointURL": "http://minio.minio.svc.cluster.local:9000",
                    "s3Credentials": {
                        "accessKeyId": {
                            "name": "s3creds",
                            "key": "MINIO_ACCESS_KEY"
                        },
                        "secretAccessKey": {
                            "name": "s3creds",
                            "key": "MINIO_SECRET_KEY"
                        }
                    }
                },
                "trunk_installs": [
                    {
                        "name": "pg_partman",
                        "version": "4.7.3",
                    },
                    {
                        "name": "pgmq",
                        "version": "0.10.0",
                    },
                    {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "pg_partman",
                        "description": "pg_partman extension",
                        "locations": [{
                            "enabled": false,
                            "version": "4.7.3",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pgmq",
                        "description": "pgmq extension",
                        "locations": [{
                            "enabled": false,
                            "version": "0.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                    {
                        "name": "pg_stat_statements",
                        "description": "pg_stat_statements extension",
                        "locations": [{
                            "enabled": false,
                            "version": "1.10.0",
                            "database": "postgres",
                            "schema": "public"}
                        ]
                    },
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&restore_coredb_json);
        let coredb_resource = restore_coredbs
            .patch(restore_name, &params, &patch)
            .await
            .unwrap();

        // Wait for CNPG Pod to be created
        let restore_pod_name = format!("{}-1", restore_name);
        pod_ready_and_running(restore_pods.clone(), restore_pod_name.clone()).await;

        let restore_pods: Api<Pod> = Api::namespaced(client.clone(), &restore_namespace);

        // Wait for CNPG Cluster to be created by looping over replicas until
        // they are in a running state
        for i in 1..=replicas {
            let restore_pod_name = format!("{}-{}", restore_name, i);
            pod_ready_and_running(restore_pods.clone(), restore_pod_name).await;
        }

        // Assert that we can query the database with \dx;
        let result =
            psql_with_retry(context.clone(), coredb_resource.clone(), "\\dx".to_string()).await;
        assert!(result.stdout.clone().unwrap().contains("plpgsql"));

        // Assert that the extensions are installed on both replicas
        let retrieved_pods_result = coredb_resource.pods_by_cluster(client.clone()).await;

        let retrieved_pods = match retrieved_pods_result {
            Ok(pods_list) => pods_list,
            Err(e) => {
                panic!("Failed to retrieve pods: {:?}", e);
            }
        };

        // Wait for pgmq to be installed before proceeding.
        trunk_install_status(&restore_coredbs, restore_name, "pgmq").await;
        for pod in &retrieved_pods {
            let cmd = vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "ls /var/lib/postgresql/data/tembo/extension/pgmq.control".to_owned(),
            ];
            let pod_name = pod.metadata.name.clone().expect("Pod should have a name");
            pod_ready_and_running(restore_pods.clone(), pod_name.clone()).await;
            let result = run_command_in_container(
                restore_pods.clone(),
                pod_name.clone(),
                cmd.clone(),
                Some("postgres".to_string()),
            )
            .await;
            println!("Pod: {}", pod_name.clone());
            println!("Result: {:?}", result);
            assert!(result.contains("pgmq.control"));
        }

        // Check to make sure the data from the original database is present
        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "SELECT * FROM test;".to_string(),
        )
        .await;
        assert!(result.stdout.clone().unwrap().contains("test"));

        // Check to make sure CoreDBStatus has last_archiver_status set to a DateTime<Utc>
        let cdbstatus = coredbs.get(name).await.unwrap();
        let last_archiver_state = cdbstatus
            .status
            .as_ref()
            .and_then(|stats| stats.last_archiver_status);
        assert!(last_archiver_state.is_some());

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;

        restore_coredbs
            .delete(restore_name, &Default::default())
            .await
            .unwrap();
        println!("Waiting for CoreDB to be deleted: {}", &restore_name);
        let _assert_coredb_deleted = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_COREDB_DELETED),
            await_condition(
                restore_coredbs.clone(),
                restore_name,
                conditions::is_deleted(""),
            ),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "CoreDB {} was not deleted after waiting {} seconds",
                restore_name, TIMEOUT_SECONDS_COREDB_DELETED
            )
        });
        println!("CoreDB resource deleted {}", restore_name);

        // Delete namespace
        let _ = delete_namespace(client.clone(), &restore_namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_pooler() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-pooler-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;
        let resources = serde_json::json!({
            "limits": {
                "cpu": "200m",
                "memory": "256Mi"
            },
            "requests": {
                "cpu": "100m",
                "memory": "128Mi"
            }
        });

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "connectionPooler": {
                    "enabled": true,
                    "pooler": {
                        "resources": resources,
                    },
                },
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        let pod_name = format!("{}-1", name);

        pod_ready_and_running(pods.clone(), pod_name.clone()).await;

        // Check for pooler
        let pooler_name = format!("{}-pooler", name);
        let poolers: Api<Pooler> = Api::namespaced(client.clone(), &namespace);
        let _pooler = poolers.get(&pooler_name).await.unwrap();
        println!("Found pooler: {}", pooler_name);

        // Check for pooler service
        let pooler_services: Api<Service> = Api::namespaced(client.clone(), &namespace);
        let _pooler_service = pooler_services.get(&pooler_name).await.unwrap();
        println!("Found pooler service: {}", pooler_name);

        // Check for pooler secret
        let pooler_secrets: Api<Secret> = Api::namespaced(client.clone(), &namespace);
        let _pooler_secret = pooler_secrets.get(&pooler_name).await.unwrap();
        println!("Found pooler secret: {}", pooler_name);

        // Check for pooler deployment
        let pooler_deployments: Api<Deployment> = Api::namespaced(client.clone(), &namespace);
        let pooler_deployment = pooler_deployments.get(&pooler_name).await.unwrap();
        println!("Found pooler deployment: {}", pooler_name);

        // Check pooler_deployment for correct resources
        let pooler_deployment_resources_json = serde_json::to_value(
            pooler_deployment
                .spec
                .unwrap()
                .template
                .spec
                .unwrap()
                .containers[0]
                .resources
                .as_ref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(pooler_deployment_resources_json, resources);

        // Check for pooler IngressRouteTCP
        let pooler_ingressroutetcps: Api<IngressRouteTCP> =
            Api::namespaced(client.clone(), &namespace);
        let _pooler_ingressroutetcp = pooler_ingressroutetcps
            .get(format!("{pooler_name}-0").as_str())
            .await
            .unwrap();
        println!("Found pooler IngressRouteTCP: {pooler_name}-0");

        // Query the database to make sure the pgbouncer role was created
        let _pgb_query = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT rolname FROM pg_roles;".to_string(),
            "cnpg_pooler_pgbouncer".to_string(),
            false,
        )
        .await;

        // Query the database to make sure the pgbouncer role has usage privilege
        let _usage_privilege_result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "SELECT has_schema_privilege('cnpg_pooler_pgbouncer', 'public', 'USAGE') AS has_usage_permission;".to_string(),
            "t".to_string(),
            false,
        )
        .await;

        // Update coredb to disable pooler
        let _coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": replicas,
                "connectionPooler": {
                    "enabled": false,
                },
            }
        });

        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for pooler to be deleted
        let _assert_pooler_deleted = tokio::time::timeout(
            Duration::from_secs(30),
            await_condition(poolers.clone(), &pooler_name, conditions::is_deleted("")),
        );
        println!("Pooler deleted: {}", pooler_name);

        // Wait for pooler service to be deleted
        let _assert_pooler_service_deleted = tokio::time::timeout(
            Duration::from_secs(30),
            await_condition(
                pooler_services.clone(),
                &pooler_name,
                conditions::is_deleted(""),
            ),
        );
        println!("Pooler service deleted: {}", pooler_name);

        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_restart_and_update_replicas() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-restart-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate basic CoreDB resource to start with
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
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        {
            let pod_name = format!("{}-1", name);

            pod_ready_and_running(pods.clone(), pod_name.clone()).await;
            wait_til_status_is_filled(&coredbs, name).await;
        }

        // Ensure status.running is true
        assert!(status_running(&coredbs, name).await);
        let initial_start_time = get_pg_start_time(&coredbs, name, context.clone()).await;

        println!("Initial start time: {}", initial_start_time);

        {
            let pod_name = format!("{}-1", name);

            pod_ready_and_running(pods.clone(), pod_name.clone()).await;
            wait_til_status_is_filled(&coredbs, name).await;
        }

        // Update CoreDB with restart annotation and replica = 2
        let replicas = 2;
        // Set restart annotation
        let restart = Utc::now()
            .to_rfc3339_opts(SecondsFormat::Secs, true)
            .to_string();
        // Update coredb to disable pooler
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name,
                "annotations": {
                    "kubectl.kubernetes.io/restartedAt": restart
                }
            },
            "spec": {
                "replicas": replicas,
            }
        });

        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for Postgres to restart
        {
            let started = Utc::now();
            let max_wait_time =
                chrono::TimeDelta::try_seconds(TIMEOUT_SECONDS_POD_READY as _).unwrap();
            let mut running_became_true = false;
            while Utc::now().signed_duration_since(started) < max_wait_time {
                if status_running(&coredbs, name).await.not() {
                    println!("status.running is still false. Retrying in 3 secs.");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                } else {
                    println!("status.running is now true once again!");

                    running_became_true = true;
                    break;
                }
            }

            assert!(
                running_became_true,
                "status.running should've become true once restarted"
            );
        }

        // Wait for new CNPG secondary Pod to be created and running
        // loop over replicas until they are both in a running state
        for i in 1..=replicas {
            let pod_name = format!("{}-{}", name, i);
            pod_ready_and_running(pods.clone(), pod_name).await;
        }

        let reboot_start_time = get_pg_start_time(&coredbs, name, context).await;
        println!("Initial start time: {}", initial_start_time);
        println!("Reboot start time: {}", reboot_start_time);

        assert!(
            reboot_start_time > initial_start_time,
            "start time should've changed"
        );

        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_cnpg_update_image_and_params() {
        // Initialize the Kubernetes client
        let client = kube_client().await;
        let state = State::default();
        let context = state.create_context(client.clone());

        // Configurations
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let name = &format!("test-cnpg-upimage-{}", suffix);
        let namespace = match create_namespace(client.clone(), name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";
        let replicas = 1;

        // Create a pod we can use to run commands in the cluster
        let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // Generate CoreDB resource with params
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
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let _coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        // Wait for CNPG Pod to be created
        {
            let pod_name = format!("{}-1", name);

            pod_ready_and_running(pods.clone(), pod_name.clone()).await;
            wait_til_status_is_filled(&coredbs, name).await;
        }

        // Ensure status.running is true
        assert!(status_running(&coredbs, name).await);

        // Update CNPG image and params
        let coredb_json = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": name
            },
            "spec": {
                "image": "quay.io/tembo/standard-cnpg:15-120cc24",
                "replicas": replicas,
                "trunk_installs": [
                    {
                        "name": "pg_partman",
                        "version": "4.7.3",
                    },
                    {
                        "name": "pgmq",
                        "version": "1.1.1",
                    },
                    {
                        "name": "pg_stat_statements",
                        "version": "1.10.0",
                    },
                    {
                        "name": "postgis",
                        "version": "3.4.0",
                    },
                ],
                "extensions": [
                    {
                        "name": "pg_partman",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "4.7.3",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "pgmq",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "1.1.1",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "pg_stat_statements",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "1.10.0",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    },
                    {
                        "name": "postgis",
                        "locations": [
                        {
                          "enabled": true,
                          "version": "3.4.0",
                          "database": "postgres",
                          "schema": "public"
                        }]
                    }
                ],
                "runtime_config": [
                    {
                        "name": "shared_preload_libraries",
                        "value": "pg_stat_statements"
                    },
                    {
                        "name": "pg_partman_bgw.interval",
                        "value": "30"
                    },
                    {
                        "name": "pg_partman_bgw.role",
                        "value": "postgres"
                    },
                    {
                        "name": "pg_partman_bgw.dbname",
                        "value": "postgres"
                    }
                ]
            }
        });

        // Use the patch method to update the Cluster resource
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&coredb_json);
        let coredb_resource = coredbs.patch(name, &params, &patch).await.unwrap();

        let _result = wait_until_psql_contains(
            context.clone(),
            coredb_resource.clone(),
            "show shared_preload_libraries;".to_string(),
            "pg_partman_bgw".to_string(),
            false,
        )
        .await;

        // Assert that shared_preload_libraries contains pg_stat_statements
        // and pg_partman_bgw

        let result = psql_with_retry(
            context.clone(),
            coredb_resource.clone(),
            "show shared_preload_libraries;".to_string(),
        )
        .await;

        let stdout = match result.stdout {
            Some(output) => output,
            None => panic!("stdout is None"),
        };

        assert!(stdout.contains("pg_partman_bgw"));
        assert!(stdout.contains("pg_stat_statements"));

        //assert to make sure the correct image has been updated
        let cluster_api: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        let cluster = cluster_api.get(name).await.unwrap();
        let image = cluster.spec.image.clone();
        assert_eq!(image, "quay.io/tembo/standard-cnpg:15-120cc24");

        // CLEANUP TEST
        // Cleanup CoreDB
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

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    #[tokio::test]
    #[ignore]
    /// Test the hibernation system
    /// Ensure that the cluster performs the following:
    ///   * Starts and is running following init
    ///   * Stops and has no pods after hibernation (spec.stop: true)
    ///   * Starts and is running after thaw (spec.stop: false)
    async fn functional_test_hibernate() {
        let test_name = "test-hibernate-cnpg";
        let test = TestCore::new(test_name).await;
        let name = test.name.clone();
        let pooler_name = format!("{}-pooler", name);

        // Generate very simple CoreDB JSON definitions. The first will be for
        // initializing and starting the cluster, and the second for stopping
        // it. We'll use a single replica to ensure _all_ parts of the cluster
        // are affected by hibernate.

        let cluster_start = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": "CoreDB",
            "metadata": {
                "name": name
            },
            "spec": {
                "replicas": 1,
                "stop": false,
                "connectionPooler": {
                    "enabled": true
                },
            }
        });

        let mut cluster_stop = cluster_start.clone();
        cluster_stop["spec"]["stop"] = serde_json::json!(true);

        // Begin by starting the cluster and validating that it worked.
        // We need this here as initial iterations of the hibernate code
        // prevented the cluster from starting.

        let _ = test.set_cluster_def(&cluster_start).await;
        assert!(status_running(&test.coredbs, &name).await);
        assert!(pooler_status_running(&test.poolers, &pooler_name).await);

        // Stop the cluster and check to make sure it's not running to ensure
        // hibernate is doing its job.

        let _ = test.set_cluster_def(&cluster_stop).await;
        let _ = wait_until_status_not_running(&test.coredbs, &name).await;
        assert!(status_running(&test.coredbs, &name).await.not());
        assert!(pooler_status_running(&test.poolers, &pooler_name)
            .await
            .not());

        // Patch the cluster to start it up again, then check to ensure it
        // actually did so. This proves hibernation can be reversed.

        println!("Starting cluster after hibernation");
        println!("CoreDB: {}", cluster_start);
        let _ = test.set_cluster_def(&cluster_start).await;
        assert!(status_running(&test.coredbs, &name).await);
        assert!(pooler_status_running(&test.poolers, &pooler_name).await);

        test.teardown().await;
    }

    #[tokio::test]
    #[ignore]
    async fn functional_test_app_service_podmonitor() {
        // Validates PodMonitor resource created and destroyed properly
        // PodMonitor created/destroyed when adding/removing `metrics` from CoredBSpec
        // PodMonitor destroyed when the corresponding AppService is deleted

        // Initialize the Kubernetes client
        let client = kube_client().await;
        let mut rng = rand::thread_rng();
        let suffix = rng.gen_range(0..100000);
        let cdb_name = &format!("test-app-service-podmon-{}", suffix);
        let namespace = match create_namespace(client.clone(), cdb_name).await {
            Ok(namespace) => namespace,
            Err(e) => {
                eprintln!("Error creating namespace: {}", e);
                std::process::exit(1);
            }
        };

        let kind = "CoreDB";

        // Apply a basic configuration of CoreDB
        println!("Creating CoreDB resource {}", cdb_name);
        let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
        // generate an instance w/ 2 appServices
        let full_app = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": cdb_name
            },
            "spec": {
                "appServices": [
                    {
                        "name": "dummy-exporter",
                        "image": "prom/blackbox-exporter",
                        "routing": [
                            {
                                "port": 9115,
                                "ingressPath": "/metrics",
                                "ingressType": "http"
                            }
                        ],
                        "metrics": {
                            "path": "/metrics",
                            "port": 9115
                        }
                    },
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&full_app);
        let _coredb_resource = coredbs.patch(cdb_name, &params, &patch).await.unwrap();

        // pause for resource creation
        use controller::prometheus::podmonitor_crd::PodMonitor;
        let pmon_name = format!("{}-dummy-exporter", cdb_name);
        let podmon = get_resource::<PodMonitor>(client.clone(), &namespace, &pmon_name, 50, true)
            .await
            .unwrap();
        let pmon_spec = podmon.spec.pod_metrics_endpoints.unwrap();
        assert_eq!(pmon_spec.len(), 1);

        // disable metrics
        let no_metrics_app = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": cdb_name
            },
            "spec": {
                "appServices": [
                    {
                        "name": "dummy-exporter",
                        "image": "prom/blackbox-exporter",
                        "routing": [
                            {
                                "port": 9115,
                                "ingressPath": "/metrics",
                                "ingressType": "http"
                            }
                        ],
                    },
                ]
            }
        });
        let params = PatchParams::apply("tembo-integration-test");
        let patch = Patch::Apply(&no_metrics_app);
        let _coredb_resource = coredbs.patch(cdb_name, &params, &patch).await.unwrap();
        let podmon =
            get_resource::<PodMonitor>(client.clone(), &namespace, &pmon_name, 50, false).await;
        assert!(podmon.is_err());

        // renable it, assert it exists, then delete the app and assert PodMonitor is gone
        let patch = Patch::Apply(&full_app);
        let _coredb_resource = coredbs.patch(cdb_name, &params, &patch).await.unwrap();
        let podmon = get_resource::<PodMonitor>(client.clone(), &namespace, &pmon_name, 50, true)
            .await
            .unwrap();

        let pmon_spec = podmon.spec.pod_metrics_endpoints.unwrap();
        assert_eq!(pmon_spec.len(), 1);
        // delete the app
        let no_app = serde_json::json!({
            "apiVersion": API_VERSION,
            "kind": kind,
            "metadata": {
                "name": cdb_name
            },
            "spec": {
                "appServices": []
            }
        });
        let patch = Patch::Apply(&no_app);
        let _coredb_resource = coredbs.patch(cdb_name, &params, &patch).await.unwrap();
        let podmon =
            get_resource::<PodMonitor>(client.clone(), &namespace, &pmon_name, 50, false).await;
        assert!(podmon.is_err());

        // CLEANUP TEST
        // Cleanup CoreDB
        coredbs.delete(cdb_name, &Default::default()).await.unwrap();
        println!("Waiting for CoreDB to be deleted: {}", &cdb_name);
        let _assert_coredb_deleted = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS_COREDB_DELETED),
            await_condition(coredbs.clone(), cdb_name, conditions::is_deleted("")),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "CoreDB {} was not deleted after waiting {} seconds",
                cdb_name, TIMEOUT_SECONDS_COREDB_DELETED
            )
        });
        println!("CoreDB resource deleted {}", cdb_name);

        // Delete namespace
        let _ = delete_namespace(client.clone(), &namespace).await;
    }

    /// Regression test for CLOUD-1015
    ///
    /// There used to be an issue figuring out the Trunk project version of one of the built-in Postgres language extensions (e.g. plpython, pltcl, plperl)
    /// given its extension version. This test should replicate that scenario.
    #[tokio::test]
    #[ignore]
    async fn functional_test_basic_cnpg_plpython() {
        let test_name = "test-basic-cnpg-plpython";
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
                        "name": "plpython3u",
                        "description": "Th PL/Python3U untrusted procedural language for PostgreSQL.",
                        "locations": [{
                            "enabled": true,
                            "version": "1.0",
                            "schema": "public",
                            "database": "postgres",
                        }],
                    }],
                // plpython is not installed through Trunk, it's built from scratch when we build Postgres with `make world`
                "trunk_installs": []
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
            "plpython3u".to_string(),
            false,
        )
        .await;

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
}
