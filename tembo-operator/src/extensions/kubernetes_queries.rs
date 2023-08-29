use crate::{
    apis::coredb_types::{CoreDB, CoreDBStatus},
    extensions::{
        database_queries::REQUIRES_LOAD,
        types::{ExtensionInstallLocationStatus, ExtensionStatus, TrunkInstallStatus},
    },
    get_current_coredb_resource, patch_cdb_status_merge, Context,
};
use kube::{runtime::controller::Action, Api};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tracing::{
    error,
    log::{debug, info},
    warn,
};

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
                if location.database == new_location_status.database
                    && location.schema == new_location_status.schema
                {
                    // Then replace it
                    *location = new_location_status.clone();
                    return new_extensions_status;
                }
            }
            // If we never found the location, append it to existing extension status
            extension.locations.push(new_location_status.clone());
            // Then sort the locations alphabetically by database and schema
            // sort locations by database and schema so the order is deterministic
            extension
                .locations
                .sort_by(|a, b| a.database.cmp(&b.database).then(a.schema.cmp(&b.schema)));
            return new_extensions_status;
        }
    }
    error!("Merging a location status into extension status list, but the extension was not found in the list. This is not expected");
    let load = REQUIRES_LOAD.contains(&extension_name);
    // If we never found the extension status, append it
    new_extensions_status.push(ExtensionStatus {
        name: extension_name.to_string(),
        description: None,
        locations: vec![new_location_status.clone()],
        create_extension: None,
        load: Some(load),
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
    let patch_status = json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "status": {
            "extensions": ext_status_updates
        }
    });
    let coredb_api: Api<CoreDB> = Api::namespaced(
        ctx.client.clone(),
        &cdb.metadata
            .namespace
            .clone()
            .expect("CoreDB should have a namespace"),
    );
    patch_cdb_status_merge(
        &coredb_api,
        &cdb.metadata
            .name
            .clone()
            .expect("CoreDB should always have a name"),
        patch_status,
    )
    .await?;
    Ok(())
}

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
    info!(
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
                name);
            vec![]
        }
        Some(trunk_installs) => trunk_installs,
    };
    info!(
        "There are currently {} trunk installs in status for {}",
        current_trunk_installs.len(),
        name
    );
    let mut new_trunk_installs_status: Vec<TrunkInstallStatus> = vec![];
    let mut trunk_install_found = false;
    // Check if the trunk install is already in the list
    for (_i, existing_trunk_install_status) in current_trunk_installs.iter().enumerate() {
        if existing_trunk_install_status.name == new_trunk_install_status_to_include.clone().name {
            warn!(
                "Trunk install {} already in status on {}, replacing.",
                &new_trunk_install_status_to_include.name, name
            );
            new_trunk_installs_status.push(new_trunk_install_status_to_include.clone());
            trunk_install_found = true;
        } else {
            new_trunk_installs_status.push(existing_trunk_install_status.clone());
        }
    }
    if !trunk_install_found {
        new_trunk_installs_status.push(new_trunk_install_status_to_include.clone());
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
        trunk_installs: Some(new_trunk_installs_status.clone()),
        ..current_status
    };
    let patch_status = json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "status": new_status
    });
    patch_cdb_status_merge(cdb, name, patch_status).await?;
    info!("Patched status to update trunk installs for {}", name);
    Ok(new_trunk_installs_status.clone())
}

#[cfg(test)]
mod tests {
    use crate::extensions::{
        kubernetes_queries::merge_location_status_into_extension_status_list,
        types::{ExtensionInstallLocationStatus, ExtensionStatus},
    };

    #[test]
    fn test_merge_existing_extension_and_location() {
        let current_extensions_status = vec![ExtensionStatus {
            name: "ext1".to_string(),
            description: None,
            locations: vec![ExtensionInstallLocationStatus {
                enabled: Some(false),
                database: "db1".to_string(),
                schema: Some("schema1".to_string()),
                version: None,
                error: Some(false),
                error_message: None,
            }],
            create_extension: None,
            load: None,
        }];
        let new_location_status = ExtensionInstallLocationStatus {
            enabled: Some(true),
            database: "db1".to_string(),
            schema: Some("schema1".to_string()),
            version: None,
            error: Some(false),
            error_message: None,
        };

        // Try updating existing from disabled to enabled
        let result = merge_location_status_into_extension_status_list(
            "ext1",
            &new_location_status,
            current_extensions_status,
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].locations.len(), 1);
        assert_eq!(result[0].locations[0].enabled, Some(true));
    }

    #[test]
    fn test_merge_existing_extension_new_location() {
        let current_extensions_status = vec![ExtensionStatus {
            name: "ext1".to_string(),
            description: None,
            locations: vec![ExtensionInstallLocationStatus {
                enabled: Some(false),
                database: "db1".to_string(),
                schema: Some("schema2".to_string()),
                version: None,
                error: Some(false),
                error_message: None,
            }],
            create_extension: None,
            load: None,
        }];
        let new_location_status = ExtensionInstallLocationStatus {
            enabled: Some(true),
            database: "db1".to_string(),
            schema: Some("schema1".to_string()),
            version: None,
            error: Some(false),
            error_message: None,
        };

        let result = merge_location_status_into_extension_status_list(
            "ext1",
            &new_location_status,
            current_extensions_status,
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].locations.len(), 2);
        assert_eq!(result[0].locations[0].database, "db1".to_string());
        assert_eq!(result[0].locations[0].schema, Some("schema1".to_string()));
        assert_eq!(result[0].locations[0].enabled, Some(true));
        assert_eq!(result[0].locations[1].database, "db1".to_string());
        assert_eq!(result[0].locations[1].schema, Some("schema2".to_string()));
        assert_eq!(result[0].locations[1].enabled, Some(false));
    }

    #[test]
    fn test_merge_new_extension_new_location() {
        let current_extensions_status = vec![ExtensionStatus {
            name: "ext2".to_string(),
            description: None,
            locations: vec![ExtensionInstallLocationStatus {
                enabled: Some(false),
                database: "db1".to_string(),
                schema: Some("schema1".to_string()),
                version: None,
                error: Some(false),
                error_message: None,
            }],
            create_extension: None,
            load: None,
        }];
        let new_location_status = ExtensionInstallLocationStatus {
            enabled: Some(true),
            database: "db1".to_string(),
            schema: Some("schema1".to_string()),
            version: None,
            error: Some(false),
            error_message: None,
        };

        let result = merge_location_status_into_extension_status_list(
            "ext1",
            &new_location_status,
            current_extensions_status,
        );

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].locations.len(), 1);
        assert_eq!(result[1].locations.len(), 1);
        assert_eq!(result[0].name, "ext1".to_string());
        assert_eq!(result[0].locations[0].database, "db1".to_string());
        assert_eq!(result[0].locations[0].schema, Some("schema1".to_string()));
        assert_eq!(result[0].locations[0].enabled, Some(true));
        assert_eq!(result[1].name, "ext2".to_string());
        assert_eq!(result[1].locations[0].database, "db1".to_string());
        assert_eq!(result[1].locations[0].schema, Some("schema1".to_string()));
        assert_eq!(result[1].locations[0].enabled, Some(false));
    }
}
