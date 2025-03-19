use crate::{
    apis::coredb_types::CoreDB,
    cloudnativepg::cnpg::{get_fenced_pods, unfence_pod},
    extensions::{
        kubernetes_queries::{add_trunk_install_to_status, remove_trunk_installs_from_status},
        types::{TrunkInstall, TrunkInstallStatus},
    },
    trunk::get_latest_trunk_project_version,
    Context,
};
use k8s_openapi::{api::core::v1::Pod, apimachinery::pkg::apis::meta::v1::ObjectMeta};
use kube::{runtime::controller::Action, Api, ResourceExt};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tracing::{debug, error, info, instrument, warn};

use crate::apis::coredb_types::CoreDBStatus;

// Syncroniously merge and deduplicate pods
#[instrument(skip(non_fenced_pods, fenced_names) fields(trace_id))]
fn merge_and_deduplicate_pods(
    non_fenced_pods: Vec<Pod>,
    fenced_names: Option<Vec<String>>,
) -> Vec<Pod> {
    let mut all_pods: Vec<Pod> = Vec::new();
    let mut unique_pod_names: HashSet<String> = HashSet::new();

    // Add non-fenced pods and update the HashSet with their names
    for pod in non_fenced_pods {
        if let Some(pod_name) = &pod.metadata.name {
            if unique_pod_names.insert(pod_name.clone()) {
                all_pods.push(pod);
            }
        }
    }

    // Add fenced pods and update the HashSet with their names
    if let Some(fenced_names) = fenced_names {
        for fenced_name in fenced_names {
            if unique_pod_names.insert(fenced_name.clone()) {
                let new_pod = Pod {
                    metadata: ObjectMeta {
                        name: Some(fenced_name),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                all_pods.push(new_pod);
            }
        }
    }

    all_pods
}

// Collect any fenced pods and add them to the list of pods to install extensions into
#[instrument(skip(ctx, cdb) fields(trace_id))]
pub async fn all_fenced_and_non_fenced_pods(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<Pod>, Action> {
    let name = cdb.name_any();

    // Get fenced pods
    let pods_fenced = get_fenced_pods(cdb, ctx.clone()).await?;

    // Get all non-fenced pods
    let non_fenced_pods = cdb.pods_by_cluster_ready_or_not(ctx.client.clone()).await?;

    // Merge and deduplicate pod names
    let all_pods = merge_and_deduplicate_pods(non_fenced_pods, pods_fenced);

    debug!(
        "After appending fenced instances for {}, pod count: {}",
        &name,
        all_pods.len()
    );

    Ok(all_pods)
}

/// Find all trunk installs to remove and return a list of strings
#[instrument(skip(cdb) fields(trace_id))]
fn find_trunk_installs_to_remove_from_status(cdb: &CoreDB) -> Vec<String> {
    let name = cdb.name_any();
    debug!(
        "Checking for trunk installs to remove from status for {}",
        &name
    );

    let mut trunk_installs_to_remove_from_status = Vec::new();

    // Get extensions in status.trunk_install that are not in spec
    // Deleting them from status allows for retrying installation
    // by first removing the extension from the spec, then adding it back
    match &cdb.status {
        None => {
            return trunk_installs_to_remove_from_status;
        }
        Some(status) => match &status.trunk_installs {
            None => {
                return trunk_installs_to_remove_from_status;
            }
            Some(trunk_installs) => {
                for ext_status in trunk_installs {
                    if !cdb
                        .spec
                        .trunk_installs
                        .iter()
                        .any(|ext| ext.name == ext_status.name)
                    {
                        trunk_installs_to_remove_from_status.push(ext_status.name.clone());
                    }
                }
            }
        },
    };

    trunk_installs_to_remove_from_status
}

/// Find all trunk installs to install on a pod and return a Vec of TrunkInstall
/// This function also needs to define a lifetime, since we are only returning a reference to
/// TrunkInstall, which is owned by CoreDB we only need to define a lifetime for CoreDB
#[instrument(skip(cdb, pod_name) fields(trace_id))]
pub fn find_trunk_installs_to_pod<'a>(cdb: &'a CoreDB, pod_name: &str) -> Vec<&'a TrunkInstall> {
    debug!(
        "Checking for trunk installs to install on pod {} for {}",
        pod_name,
        cdb.name_any()
    );

    let pod_name = pod_name.to_owned();
    let mut trunk_installs_to_install = Vec::new();

    // Get extensions in spec.trunk_install that are not in status.trunk_install
    for ext in &cdb.spec.trunk_installs {
        // All TrunkInstallStatus in CDB spec
        let trunk_install_statuses = cdb
            .status
            .as_ref()
            .and_then(|status| status.trunk_installs.as_deref())
            .unwrap_or_default();

        if !trunk_install_statuses.iter().any(|ext_status| {
            ext.name == ext_status.name
                && !ext_status.error
                && ext_status
                    .installed_to_pods
                    .as_deref()
                    .unwrap_or_default()
                    .contains(&pod_name)
        }) {
            trunk_installs_to_install.push(ext);
        }
    }

    trunk_installs_to_install
}

// is_pod_fenced function checks if a pod is fenced and returns a bool or requeue action
#[instrument(skip(cdb, ctx, pod_name) fields(trace_id, pod_name))]
async fn is_pod_fenced(cdb: &CoreDB, ctx: Arc<Context>, pod_name: &str) -> Result<bool, Action> {
    let coredb_name = cdb.metadata.name.as_deref().unwrap_or_default();

    debug!(
        "Checking if pod {} is fenced for instance {}",
        pod_name, coredb_name
    );

    let fenced_pods = get_fenced_pods(cdb, ctx.clone()).await?;

    if let Some(fenced_pods) = fenced_pods {
        // Check if pod_name is in fenced_pods
        if fenced_pods.contains(&pod_name.to_string()) {
            debug!("Instance {} pod {} is fenced", coredb_name, pod_name);
            return Ok(true);
        }
    }

    Ok(false)
}

#[instrument(skip(ctx, cdb))]
pub async fn reconcile_trunk_installs(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<TrunkInstallStatus>, Action> {
    let instance_name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!(
            "CoreDB namespace is empty for instance: {}.",
            &instance_name
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    debug!("Starting to reconcile trunk installs for {}", instance_name);

    let coredb_api: Api<CoreDB> = Api::namespaced(ctx.client.clone(), namespace);

    // Get extensions in status.trunk_install that are not in spec
    // Deleting them from status allows for retrying installation
    // by first removing the extension from the spec, then adding it back
    // Get trunk installs to remove from status
    let trunk_installs_to_remove_from_status = find_trunk_installs_to_remove_from_status(cdb);

    // Remove extensions from status
    remove_trunk_installs_from_status(
        &coredb_api,
        &instance_name,
        trunk_installs_to_remove_from_status,
    )
    .await?;

    // Get extensions in spec.trunk_install that are not in status.trunk_install
    let mut all_results = Vec::new();

    // Get all fenced and non-fenced pods for this instance
    let all_pods = all_fenced_and_non_fenced_pods(cdb, ctx.clone()).await?;

    // Loop through all pods and install missing trunk installs
    for pod in all_pods {
        let pod_name = pod.metadata.name.expect("Pod should always have a name");

        // Filter trunk installs that are not yet installed on this instance
        let trunk_installs_to_pod = find_trunk_installs_to_pod(cdb, &pod_name);

        if trunk_installs_to_pod.is_empty() {
            debug!(
                "Unfencing any pod that does not require trunk installs, pod {} for {}",
                pod_name, instance_name
            );
            // Check if pod is fenced and if so unfence it otherwise continue
            if is_pod_fenced(cdb, ctx.clone(), &pod_name).await? {
                // Unfence pod_name
                unfence_pod(cdb, ctx.clone(), &pod_name.clone()).await?;
            }
            continue;
        }

        // Install missing trunk installs
        match install_extensions_to_pod(cdb, trunk_installs_to_pod, &ctx, pod_name.clone()).await {
            Ok(result) => {
                all_results = result;
            }
            Err(err) => return Err(err),
        };
    }

    info!(
        "Completed trunk install reconciliation for instance {}",
        instance_name
    );

    // Check if all_results is empty, if so use status.trunk_installs to make sure we don't end up
    // in a reconcile loop and re-install loop
    if all_results.is_empty() {
        debug!("No trunk installs to reconcile for {}", instance_name);
        all_results = cdb
            .status
            .clone()
            .unwrap_or_default()
            .trunk_installs
            .clone()
            .unwrap_or_default();
    }
    Ok(all_results)
}

// initializing current_trunk_install_statuses from CoreDB status and return a Vec of TrunkInstallStatus
#[instrument(skip(cdb, coredb_name) fields(trace_id))]
fn initialize_trunk_install_statuses(cdb: &CoreDB, coredb_name: &str) -> Vec<TrunkInstallStatus> {
    cdb.status
        .clone()
        .unwrap_or_else(|| {
            debug!("No current status on {}, initializing default", coredb_name);
            CoreDBStatus::default()
        })
        .trunk_installs
        .unwrap_or_else(|| {
            debug!(
                "No current trunk installs on {}, initializing empty list",
                coredb_name
            );
            vec![]
        })
}

/// execute_extension_install_command function executes the trunk install command and returns a
/// TrunkInstallStatus or bool
#[instrument(skip(cdb, ctx, coredb_name, ext, pod_name) fields(trace_id))]
async fn execute_extension_install_command(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    coredb_name: &str,
    ext: &TrunkInstall,
    pod_name: &str,
) -> Result<TrunkInstallStatus, bool> {
    let client = ctx.client.clone();

    // Handle the case where version is None
    let version = match &ext.version {
        None => {
            warn!(
                "Version for extension {} not specified in {}, will fetch latest version",
                ext.name, coredb_name
            );

            match get_latest_trunk_project_version(&ext.name).await {
                Ok(latest_version) => latest_version,
                Err(_) => {
                    error!(
                        "Failed to get latest version for extension {} in {}",
                        ext.name, coredb_name
                    );

                    return Ok(TrunkInstallStatus {
                        name: ext.name.clone(),
                        version: None,
                        error: true,
                        loading: false,
                        error_message: Some("Missing version".to_string()),
                        installed_to_pods: Some(vec![pod_name.to_string()]),
                    });
                }
            }
        }
        Some(version) => version.clone(),
    };

    let cmd = vec![
        "trunk".to_owned(),
        "install".to_owned(),
        "-r https://registry.pgtrunk.io".to_owned(),
        ext.name.clone(),
        "--version".to_owned(),
        version,
        // "--pkglibdir".to_owned(),
        // cdb.spec.module_dir(),
    ];

    // If the pod is not up yet, do not try and install the extension
    if let Err(e) = cdb.log_pod_status(client.clone(), pod_name).await {
        warn!(
            "Could not fetch or log pod status for instance {}: {:?}",
            coredb_name, e
        );
        return Err(true);
    }

    let result = cdb.exec(pod_name.to_string(), client.clone(), &cmd).await;

    // Check if the exec command was successful
    // keep in mind that installed_to_pods can be merged with existing pods in the list where
    // the extension was already installed
    match result {
        Ok(result) => {
            let output = format!(
                "{}\n{}",
                result
                    .stdout
                    .unwrap_or_else(|| "Nothing in stdout".to_string()),
                result
                    .stderr
                    .unwrap_or_else(|| "Nothing in stderr".to_string())
            );

            let trunk_install_status = if result.success {
                info!(
                    "Installed extension {} into {} for {}",
                    &ext.name, pod_name, coredb_name
                );
                TrunkInstallStatus {
                    name: ext.name.clone(),
                    version: ext.version.clone(),
                    error: false,
                    loading: false,
                    error_message: None,
                    installed_to_pods: Some(vec![pod_name.to_string()]),
                }
            } else {
                error!(
                    "Failed to install extension {} into {}:\n{}",
                    &ext.name, pod_name, output
                );
                TrunkInstallStatus {
                    name: ext.name.clone(),
                    version: ext.version.clone(),
                    error: true,
                    error_message: Some(output),
                    loading: false,
                    installed_to_pods: Some(vec![pod_name.to_string()]),
                }
            };

            Ok(trunk_install_status)
        }
        Err(_) => {
            error!(
                "Kube exec error installing extension {} into {}: {}",
                &ext.name, coredb_name, "Kube exec error"
            );
            Err(true)
        }
    }
}

// Check if <extension_name>.so file exists for a given extension in `cdb.module_dir()`.
#[instrument(skip(cdb, ctx, pod_name) fields(trace_id))]
pub async fn check_for_so_files(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    pod_name: &str,
    extension_name: String,
) -> Result<bool, Action> {
    let coredb_name = cdb.metadata.name.as_deref().unwrap_or_default();

    info!(
        "Checking for {}.so in filesystem for instance {}",
        extension_name, coredb_name
    );

    let client = ctx.client.clone();

    // Check if the pod is up yet
    if let Err(e) = cdb.log_pod_status(client.clone(), pod_name).await {
        warn!(
            "Could not fetch or log pod status for instance {}: {:?}",
            coredb_name, e
        );
        return Err(Action::requeue(Duration::from_secs(10)));
    }

    let so = format!("{}/{}.so", cdb.spec.module_dir(), extension_name);
    let cmd = vec![
        "/bin/bash".to_string(),
        "-c".to_string(),
        format!("if [ -f '{so}' ]; then echo '{so}'; fi"),
    ];

    let result = cdb.exec(pod_name.to_string(), client.clone(), &cmd).await;

    match result {
        Ok(result) => {
            let output = format!(
                "{}\n{}",
                result
                    .stdout
                    .unwrap_or_else(|| "Nothing in stdout".to_string()),
                result
                    .stderr
                    .unwrap_or_else(|| "Nothing in stderr".to_string())
            );

            if result.success {
                // Check if .so files exist in output
                if output.contains(format!("{}.so", extension_name).as_str()) {
                    info!(
                        "Found {}.so file in filesystem for instance {}",
                        extension_name, coredb_name
                    );
                    return Ok(true);
                }
                info!(
                    "No {}.so found in filesystem for instance {}",
                    extension_name, coredb_name
                );
                return Ok(false);
            }
            error!(
                "Failed to check for {}.so in filesystem for instance {}:\n{}",
                extension_name, coredb_name, output
            );
            Err(Action::requeue(Duration::from_secs(10)))
        }
        Err(_) => {
            error!(
                "Kube exec error checking for {}.so file in filesystem for instance for {}",
                extension_name, coredb_name
            );
            Err(Action::requeue(Duration::from_secs(10)))
        }
    }
}

/// handles installing extensions
#[instrument(skip(ctx, cdb) fields(trace_id))]
pub async fn install_extensions_to_pod(
    cdb: &CoreDB,
    trunk_installs: Vec<&TrunkInstall>,
    ctx: &Arc<Context>,
    pod_name: String,
) -> Result<Vec<TrunkInstallStatus>, Action> {
    let coredb_name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("CoreDB namespace is empty for instance: {}.", &coredb_name);
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    let coredb_api: Api<CoreDB> = Api::namespaced(ctx.client.clone(), namespace);

    // Lookup current status for trunk installs
    let mut current_trunk_install_statuses = initialize_trunk_install_statuses(cdb, &coredb_name);

    if trunk_installs.is_empty() {
        debug!("No extensions to install into {}", coredb_name);
        return Ok(current_trunk_install_statuses);
    }
    info!(
        "Installing extensions into {}: {:?}",
        coredb_name, trunk_installs
    );

    let mut requeue = false;
    for ext in trunk_installs.iter() {
        info!(
            "Attempting to install extension: {} on {}",
            ext.name, coredb_name
        );

        // Execute trunk install command
        match execute_extension_install_command(cdb, ctx.clone(), &coredb_name, ext, &pod_name)
            .await
        {
            Ok(trunk_install_status) => {
                if trunk_install_status.error {
                    // Log and continue to the next iteration
                    warn!(
                        "Error occurred during installation: {:?}",
                        trunk_install_status.error_message
                    );
                    current_trunk_install_statuses = add_trunk_install_to_status(
                        &coredb_api,
                        &coredb_name,
                        &trunk_install_status,
                    )
                    .await?;
                    continue;
                }
                current_trunk_install_statuses =
                    add_trunk_install_to_status(&coredb_api, &coredb_name, &trunk_install_status)
                        .await?;
            }
            Err(should_requeue) => {
                requeue = should_requeue;
            }
        }
    }
    if requeue {
        warn!("Requeueing due to errors for instance {}", coredb_name);
        return Err(Action::requeue(Duration::from_secs(10)));
    }
    info!("Successfully installed all extensions to {}", pod_name);

    // Check for fenced pods and unfence it
    let fenced_pods = get_fenced_pods(cdb, ctx.clone()).await?;
    if let Some(fenced_pods) = fenced_pods {
        // Check if pod_name is in fenced_pods
        if fenced_pods.contains(&pod_name) {
            // Unfence pod_name
            unfence_pod(cdb, ctx.clone(), &pod_name.clone()).await?;
        }
    }

    Ok(current_trunk_install_statuses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apis::coredb_types::CoreDBSpec;

    #[test]
    fn test_merge_and_deduplicate_pods() {
        let pod1 = Pod {
            metadata: ObjectMeta {
                name: Some("pod1".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let pod2 = Pod {
            metadata: ObjectMeta {
                name: Some("pod2".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let non_fenced_pods = vec![pod1.clone(), pod2.clone()];
        let fenced_names = Some(vec!["pod2".to_string(), "pod3".to_string()]);

        let result = merge_and_deduplicate_pods(non_fenced_pods, fenced_names);

        // Deduplicated names should be ["pod1", "pod2", "pod3"]
        let deduplicated_names: Vec<String> = result
            .iter()
            .filter_map(|pod| pod.metadata.name.clone())
            .collect();
        assert_eq!(
            deduplicated_names,
            vec!["pod1".to_string(), "pod2".to_string(), "pod3".to_string()]
        );
    }

    #[test]
    fn test_find_trunk_installs_to_remove_from_status() {
        // Arrange
        let trunk_install1 = TrunkInstall {
            name: "install1".to_string(),
            version: Some("1.0".to_string()),
        };
        let trunk_install2 = TrunkInstall {
            name: "install2".to_string(),
            version: Some("1.0".to_string()),
        };

        let trunk_install_status1 = TrunkInstallStatus {
            name: "install1".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            error_message: None,
            loading: false,
            installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
        };

        let trunk_install_status2 = TrunkInstallStatus {
            name: "install2".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            error_message: None,
            loading: false,
            installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
        };

        let trunk_install_status3 = TrunkInstallStatus {
            name: "install3".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            loading: false,
            error_message: None,
            installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
        };

        let cdb = CoreDB {
            metadata: ObjectMeta {
                name: Some("coredb1".to_string()),
                ..Default::default()
            },
            spec: CoreDBSpec {
                trunk_installs: vec![trunk_install1.clone(), trunk_install2.clone()],
                ..Default::default()
            },
            status: Some(CoreDBStatus {
                trunk_installs: Some(vec![
                    trunk_install_status1.clone(),
                    trunk_install_status2.clone(),
                    trunk_install_status3.clone(),
                ]),
                ..Default::default()
            }),
        };

        // Act
        let result = find_trunk_installs_to_remove_from_status(&cdb);

        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "install3");
    }

    #[test]
    fn test_find_trunk_installs_to_pod() {
        // Arrange
        let trunk_install1 = TrunkInstall {
            name: "install1".to_string(),
            version: Some("1.0".to_string()),
        };
        let trunk_install2 = TrunkInstall {
            name: "install2".to_string(),
            version: Some("1.0".to_string()),
        };
        let trunk_install3 = TrunkInstall {
            name: "install3".to_string(),
            version: Some("1.0".to_string()),
        };

        let trunk_install_status1 = TrunkInstallStatus {
            name: "install1".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            error_message: None,
            loading: false,
            installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
        };

        let cdb = CoreDB {
            metadata: ObjectMeta {
                name: Some("coredb1".to_string()),
                ..Default::default()
            },
            spec: CoreDBSpec {
                trunk_installs: vec![
                    trunk_install1.clone(),
                    trunk_install2.clone(),
                    trunk_install3.clone(),
                ],
                ..Default::default()
            },
            status: Some(CoreDBStatus {
                trunk_installs: Some(vec![trunk_install_status1.clone()]),
                ..Default::default()
            }),
        };
        let pod_name = "test-coredb-24631-1";

        // Act
        let result = find_trunk_installs_to_pod(&cdb, pod_name);

        // Assert
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "install2");
        assert_eq!(result[1].name, "install3");
    }

    #[test]
    fn test_initialize_trunk_install_statuses() {
        // Test when TrunkInstallStatus should have 2
        let trunk_install1 = TrunkInstall {
            name: "install1".to_string(),
            version: Some("1.0".to_string()),
        };
        let trunk_install2 = TrunkInstall {
            name: "install2".to_string(),
            version: Some("1.0".to_string()),
        };

        let trunk_install_status1 = TrunkInstallStatus {
            name: "install1".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            loading: false,
            error_message: None,
            installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
        };

        let trunk_install_status2 = TrunkInstallStatus {
            name: "install2".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            loading: false,
            error_message: None,
            installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
        };

        let cdb_with_status = CoreDB {
            metadata: ObjectMeta {
                name: Some("coredb1".to_string()),
                ..Default::default()
            },
            spec: CoreDBSpec {
                trunk_installs: vec![trunk_install1.clone(), trunk_install2.clone()],
                ..Default::default()
            },
            status: Some(CoreDBStatus {
                trunk_installs: Some(vec![
                    trunk_install_status1.clone(),
                    trunk_install_status2.clone(),
                ]),
                ..Default::default()
            }),
        };

        let coredb_name = "cdb_with_status";
        let result = initialize_trunk_install_statuses(&cdb_with_status, coredb_name);
        assert_eq!(result.len(), 2); // as we have 2 TrunkInstallStatus objects

        // Test when CoreDB has a status but no trunk_installs
        let cdb_with_empty_status = CoreDB {
            metadata: ObjectMeta {
                name: Some("coredb1".to_string()),
                ..Default::default()
            },
            spec: CoreDBSpec {
                trunk_installs: vec![],
                ..Default::default()
            },
            status: Some(CoreDBStatus {
                trunk_installs: Some(vec![]),
                ..Default::default()
            }),
        };
        let coredb_name = "cdb_with_empty_status";
        let result = initialize_trunk_install_statuses(&cdb_with_empty_status, coredb_name);
        assert!(result.is_empty());

        // Test when CoreDB has no status
        let cdb_without_status = CoreDB {
            metadata: ObjectMeta {
                name: Some("coredb1".to_string()),
                ..Default::default()
            },
            spec: CoreDBSpec {
                ..Default::default()
            },
            status: None,
        };
        let coredb_name = "cdb_without_status";
        let result = initialize_trunk_install_statuses(&cdb_without_status, coredb_name);
        assert!(result.is_empty());
    }
}
