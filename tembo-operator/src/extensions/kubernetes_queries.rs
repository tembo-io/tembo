use crate::{
    apis::coredb_types::{CoreDB, CoreDBStatus},
    extensions::types::{ExtensionInstallLocationStatus, ExtensionStatus, TrunkInstallStatus},
    get_current_coredb_resource, patch_cdb_status_merge, Context,
};
use kube::{runtime::controller::Action, Api, ResourceExt};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tracing::{debug, error, info, instrument, warn};

pub async fn update_extension_location_in_status(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    extension_name: &str,
    new_location_status: &ExtensionInstallLocationStatus,
) -> Result<Vec<ExtensionStatus>, Action> {
    let cdb = get_current_coredb_resource(cdb, ctx.clone()).await?;
    let current_extensions_status = match &cdb.status {
        None => {
            error!("status should always already be present when merging one extension location into existing status");
            return Err(Action::requeue(Duration::from_secs(300)));
        }
        Some(status) => match &status.extensions {
            None => {
                error!("status.extensions should always already be present when merging one extension location into existing status");
                return Err(Action::requeue(Duration::from_secs(300)));
            }
            Some(extensions) => extensions.clone(),
        },
    };
    let new_extensions_status = merge_location_status_into_extension_status_list(
        extension_name,
        new_location_status,
        current_extensions_status,
    );
    update_extensions_status(&cdb, new_extensions_status.clone(), &ctx).await?;
    Ok(new_extensions_status.clone())
}

// Given a location status, set it in a provided list of extension statuses,
// replacing the current value if found, or creating the location and / or extension
// if not found.
pub fn merge_location_status_into_extension_status_list(
    extension_name: &str,
    new_location_status: &ExtensionInstallLocationStatus,
    current_extensions_status: Vec<ExtensionStatus>,
) -> Vec<ExtensionStatus> {
    let mut new_extensions_status = current_extensions_status.clone();
    for extension in &mut new_extensions_status {
        // If the extension is already in the status list
        if extension.name == extension_name {
            for location in &mut extension.locations {
                // If the location is already in the status list
                if location.database == new_location_status.database {
                    // Then replace it
                    *location = new_location_status.clone();
                    return new_extensions_status;
                }
            }
            // If we never found the location, append it to existing extension status
            extension.locations.push(new_location_status.clone());
            // Then sort the locations alphabetically by database name
            // sort locations by database and schema so the order is deterministic
            extension
                .locations
                .sort_by(|a, b| a.database.cmp(&b.database));
            return new_extensions_status;
        }
    }
    // If we never found the extension status, append it
    new_extensions_status.push(ExtensionStatus {
        name: extension_name.to_string(),
        description: None,
        locations: vec![new_location_status.clone()],
    });
    // Then sort alphabetically by name
    new_extensions_status.sort_by(|a, b| a.name.cmp(&b.name));
    new_extensions_status
}

pub async fn update_extensions_status(
    cdb: &CoreDB,
    ext_status_updates: Vec<ExtensionStatus>,
    ctx: &Arc<Context>,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("CoreDB namespace is empty for instance: {}.", &name);
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let patch_status = json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "status": {
            "extensions": ext_status_updates
        }
    });
    let coredb_api: Api<CoreDB> = Api::namespaced(ctx.client.clone(), namespace);
    patch_cdb_status_merge(&coredb_api, &name, patch_status).await?;
    Ok(())
}

#[instrument(skip(cdb))]
pub async fn remove_trunk_installs_from_status(
    cdb: &Api<CoreDB>,
    name: &str,
    trunk_install_names: Vec<String>,
) -> crate::Result<(), Action> {
    if trunk_install_names.is_empty() {
        debug!("No trunk installs to remove from status on {}", name);
        return Ok(());
    }
    info!(
        "Removing trunk installs {:?} from status on {}",
        trunk_install_names, name
    );
    let current_coredb = cdb.get(name).await.map_err(|e| {
        error!("Error getting CoreDB: {:?}", e);
        Action::requeue(Duration::from_secs(10))
    })?;
    let current_status = match current_coredb.status {
        None => {
            warn!(
                "Did not find current status, initializing an empty status {}",
                name
            );
            CoreDBStatus::default()
        }
        Some(status) => status,
    };
    let current_trunk_installs = match current_status.trunk_installs {
        None => {
            warn!(
                "Trunk installs on status is None for {}, but we are trying remove from status {:?}",
                name, trunk_install_names
            );
            return Ok(());
        }
        Some(trunk_installs) => trunk_installs,
    };
    if current_trunk_installs.is_empty() {
        warn!(
            "No trunk installs in status is an empty list {}, but we are trying remove from status {:?}",
            name, trunk_install_names
        );
        return Ok(());
    } else {
        info!(
            "There are currently {} trunk installs in status, and we are removing {} for {}",
            current_trunk_installs.len(),
            trunk_install_names.len(),
            name
        );
    }
    let mut new_trunk_installs_status = current_trunk_installs.clone();

    // Remove the trunk installs from the status
    for trunk_install_name in trunk_install_names {
        new_trunk_installs_status.retain(|t| t.name != trunk_install_name);
    }

    // sort alphabetically by name
    new_trunk_installs_status.sort_by(|a, b| a.name.cmp(&b.name));
    // remove duplicates
    new_trunk_installs_status.dedup_by(|a, b| a.name == b.name);

    info!(
        "The new status will have {} trunk installs: {}",
        new_trunk_installs_status.len(),
        name
    );
    let new_status = CoreDBStatus {
        trunk_installs: Some(new_trunk_installs_status),
        ..current_status
    };
    let patch_status = json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "status": new_status
    });
    patch_cdb_status_merge(cdb, name, patch_status).await?;
    info!("Patched status for {}", name);
    Ok(())
}

pub async fn add_trunk_install_to_status(
    cdb: &Api<CoreDB>,
    name: &str,
    new_trunk_install_status_to_include: &TrunkInstallStatus,
) -> crate::Result<Vec<TrunkInstallStatus>, Action> {
    debug!(
        "Adding trunk install {:?} to status on {}",
        new_trunk_install_status_to_include, name
    );

    let current_coredb = cdb.get(name).await.map_err(|e| {
        error!("Error getting CoreDB: {:?}", e);
        Action::requeue(Duration::from_secs(10))
    })?;

    let current_status = match current_coredb.status {
        None => {
            warn!(
                "While adding trunk install, did not find current status, initializing an empty status {}",
                name
            );
            CoreDBStatus::default()
        }
        Some(status) => status,
    };

    let current_trunk_installs = match current_status.trunk_installs {
        None => {
            warn!(
                "While adding trunk install, trunk installs on status is None for {}, initializing an empty list",
                name
            );
            vec![]
        }
        Some(trunk_installs) => trunk_installs,
    };

    info!(
        "There are currently {} trunk installs in status for {}",
        current_trunk_installs.len(),
        name
    );

    let updated_trunk_installs_status =
        update_trunk_installs(current_trunk_installs, new_trunk_install_status_to_include);

    info!(
        "The new status will have {} trunk installs: {}",
        updated_trunk_installs_status.len(),
        name
    );

    let new_status = CoreDBStatus {
        trunk_installs: Some(updated_trunk_installs_status.clone()),
        ..current_status
    };

    let patch_status = json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "status": new_status
    });

    patch_cdb_status_merge(cdb, name, patch_status).await?;

    Ok(updated_trunk_installs_status)
}

fn update_trunk_installs(
    current_trunk_installs: Vec<TrunkInstallStatus>,
    new_trunk_install: &TrunkInstallStatus,
) -> Vec<TrunkInstallStatus> {
    let mut updated_trunk_installs: Vec<TrunkInstallStatus> = vec![];

    for existing_status in &current_trunk_installs {
        if existing_status.name == new_trunk_install.name
            && existing_status.version == new_trunk_install.version
        {
            // Update existing status
            let mut update_status = existing_status.clone();
            if update_status.installed_to_pods.is_none() {
                update_status.installed_to_pods = Some(vec![]);
            }
            if let Some(ref mut installed_to_pods) = update_status.installed_to_pods {
                if let Some(new_instances) = &new_trunk_install.installed_to_pods {
                    installed_to_pods.extend_from_slice(new_instances);
                    installed_to_pods.sort();
                    installed_to_pods.dedup();
                }
            }
            updated_trunk_installs.push(update_status);
        } else {
            updated_trunk_installs.push(existing_status.clone());
        }
    }

    // If the trunk install status was not found, add it
    if !updated_trunk_installs
        .iter()
        .any(|status| status.name == new_trunk_install.name)
    {
        updated_trunk_installs.push(new_trunk_install.clone());
    }

    // sort alphabetically by name
    updated_trunk_installs.sort_by(|a, b| a.name.cmp(&b.name));
    updated_trunk_installs.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_trunk_installs_from_no_pods() {
        let current_trunk_installs = vec![TrunkInstallStatus {
            name: "pg_stat_statements".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            error_message: None,
            loading: false,
            installed_to_pods: None,
        }];
        let new_trunk_install = TrunkInstallStatus {
            name: "pg_stat_statements".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            error_message: None,
            loading: false,
            installed_to_pods: Some(vec!["pod-1".to_string(), "pod-2".to_string()]),
        };

        let updated_trunk_installs =
            update_trunk_installs(current_trunk_installs, &new_trunk_install);

        assert_eq!(updated_trunk_installs.clone().len(), 1);
        assert_eq!(
            updated_trunk_installs[0]
                .installed_to_pods
                .clone()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn test_add_new_trunk_install_with_same_name_new_host() {
        let initial_trunk_installs = vec![TrunkInstallStatus {
            error: false,
            installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
            name: "test_name".to_string(),
            version: Some("1.0.0".to_string()),
            loading: false,
            error_message: None,
        }];

        let new_trunk_install = TrunkInstallStatus {
            error: false,
            installed_to_pods: Some(vec!["test-coredb-24631-2".to_string()]),
            name: "test_name".to_string(),
            version: Some("1.0.0".to_string()),
            loading: false,
            error_message: None,
        };

        let updated_trunk_installs =
            update_trunk_installs(initial_trunk_installs.clone(), &new_trunk_install);

        assert_eq!(
            updated_trunk_installs[0].installed_to_pods,
            Some(vec![
                "test-coredb-24631-1".to_string(),
                "test-coredb-24631-2".to_string(),
            ])
        );
    }

    #[test]
    fn test_add_new_trunk_install_with_diff_names_new_host() {
        let initial_trunk_installs = vec![
            TrunkInstallStatus {
                error: false,
                installed_to_pods: Some(vec![
                    "test-coredb-24631-1".to_string(),
                    "test-coredb-24631-2".to_string(),
                ]),
                name: "test_name".to_string(),
                version: Some("1.0.0".to_string()),
                loading: false,
                error_message: None,
            },
            TrunkInstallStatus {
                error: false,
                installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
                name: "test_name2".to_string(),
                version: Some("1.0.0".to_string()),
                loading: false,
                error_message: None,
            },
        ];

        let new_trunk_install = TrunkInstallStatus {
            error: false,
            installed_to_pods: Some(vec!["test-coredb-24631-2".to_string()]),
            name: "test_name2".to_string(),
            version: Some("1.0.0".to_string()),
            loading: false,
            error_message: None,
        };

        let updated_trunk_installs =
            update_trunk_installs(initial_trunk_installs.clone(), &new_trunk_install);

        assert_eq!(
            updated_trunk_installs[0].installed_to_pods,
            Some(vec![
                "test-coredb-24631-1".to_string(),
                "test-coredb-24631-2".to_string(),
            ])
        );
        assert_eq!(
            updated_trunk_installs[1].installed_to_pods,
            Some(vec![
                "test-coredb-24631-1".to_string(),
                "test-coredb-24631-2".to_string(),
            ])
        );
    }

    #[test]
    fn test_add_new_trunk_install_test2() {
        let initial_trunk_installs = vec![
            TrunkInstallStatus {
                error: false,
                installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
                name: "pg_partman".to_string(),
                version: Some("4.7.3".to_string()),
                error_message: None,
                loading: false,
            },
            TrunkInstallStatus {
                error: false,
                installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
                name: "pg_stat_statements".to_string(),
                version: Some("1.10.0".to_string()),
                error_message: None,
                loading: false,
            },
            TrunkInstallStatus {
                error: false,
                installed_to_pods: Some(vec!["test-coredb-24631-1".to_string()]),
                name: "pgmq".to_string(),
                version: Some("0.10.0".to_string()),
                error_message: None,
                loading: false,
            },
        ];

        let new_trunk_install = TrunkInstallStatus {
            error: false,
            installed_to_pods: Some(vec!["test-coredb-24631-2".to_string()]),
            name: "pg_partman".to_string(),
            version: Some("4.7.3".to_string()),
            error_message: None,
            loading: false,
        };

        let updated_trunk_installs =
            update_trunk_installs(initial_trunk_installs.clone(), &new_trunk_install);

        println!("updated_trunk_installs: {:?}", updated_trunk_installs);

        assert_eq!(
            updated_trunk_installs[0].installed_to_pods,
            Some(vec![
                "test-coredb-24631-1".to_string(),
                "test-coredb-24631-2".to_string(),
            ])
        );

        let new_trunk_install = TrunkInstallStatus {
            error: false,
            installed_to_pods: Some(vec!["test-coredb-24631-2".to_string()]),
            name: "pg_stat_statements".to_string(),
            version: Some("1.10.0".to_string()),
            error_message: None,
            loading: false,
        };

        let updated_trunk_installs =
            update_trunk_installs(updated_trunk_installs.clone(), &new_trunk_install);

        println!("updated_trunk_installs: {:?}", updated_trunk_installs);

        assert_eq!(
            updated_trunk_installs[1].installed_to_pods,
            Some(vec![
                "test-coredb-24631-1".to_string(),
                "test-coredb-24631-2".to_string(),
            ])
        );

        let new_trunk_install = TrunkInstallStatus {
            error: false,
            installed_to_pods: Some(vec!["test-coredb-24631-2".to_string()]),
            name: "pgmq".to_string(),
            version: Some("0.10.0".to_string()),
            error_message: None,
            loading: false,
        };

        let updated_trunk_installs =
            update_trunk_installs(updated_trunk_installs.clone(), &new_trunk_install);

        println!("updated_trunk_installs: {:?}", updated_trunk_installs);

        assert_eq!(
            updated_trunk_installs[2].installed_to_pods,
            Some(vec![
                "test-coredb-24631-1".to_string(),
                "test-coredb-24631-2".to_string(),
            ])
        );
    }
}
