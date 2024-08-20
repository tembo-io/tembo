use crate::{
    apis::coredb_types::CoreDB,
    extensions::{
        database_queries, kubernetes_queries,
        types::{self, Extension, ExtensionInstallLocationStatus, ExtensionStatus},
    },
    get_current_coredb_resource,
    trunk::{self, Version},
    Context,
};
use kube::runtime::controller::Action;

use crate::extensions::install::check_for_so_files;
use crate::extensions::types::TrunkInstall;
use crate::trunk::{
    get_latest_trunk_project_version, get_loadable_library_name, get_trunk_project_description,
    get_trunk_project_for_extension, get_trunk_project_metadata_for_version,
    is_control_file_absent,
};
use crate::{
    apis::coredb_types::CoreDBStatus,
    extensions::{
        database_queries::list_shared_preload_libraries,
        kubernetes_queries::merge_location_status_into_extension_status_list,
        types::get_location_status,
    },
    trunk::extensions_that_require_load,
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tracing::{error, info, warn};

use super::database_queries::ToggleError;

pub async fn reconcile_extension_toggle_state(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<ExtensionStatus>, Action> {
    let mut all_actually_installed_extensions =
        database_queries::get_all_extensions(cdb, ctx.clone()).await?;

    // Some extensions need to be enabled with LOAD (example: auto_explain). These extensions won't show up in
    // pg_available_extensions, and therefore won't be in all_actually_installed_extensions. We need to check for
    // these extensions and add them to all_actually_installed_extensions so they are handled appropriately.
    let mut extensions_with_load = check_for_extensions_enabled_with_load(
        cdb,
        ctx.clone(),
        all_actually_installed_extensions.clone(),
    )
    .await?;
    all_actually_installed_extensions.append(&mut extensions_with_load);

    let ext_status_updates =
        determine_updated_extensions_status(cdb, all_actually_installed_extensions);
    kubernetes_queries::update_extensions_status(cdb, ext_status_updates.clone(), &ctx).await?;
    let cdb = get_current_coredb_resource(cdb, ctx.clone()).await?;
    let toggle_these_extensions = determine_extension_locations_to_toggle(&cdb);

    let ext_status_updates =
        toggle_extensions(ctx, ext_status_updates, &cdb, toggle_these_extensions).await?;
    Ok(ext_status_updates)
}

async fn toggle_extensions(
    ctx: Arc<Context>,
    ext_status_updates: Vec<ExtensionStatus>,
    cdb: &CoreDB,
    toggle_these_extensions: Vec<Extension>,
) -> Result<Vec<ExtensionStatus>, Action> {
    let current_shared_preload_libraries = list_shared_preload_libraries(cdb, ctx.clone()).await?;
    let requires_load =
        extensions_that_require_load(ctx.client.clone(), &cdb.metadata.namespace.clone().unwrap())
            .await?;
    let mut ext_status_updates = ext_status_updates.clone();

    for extension_to_toggle in toggle_these_extensions {
        for location_to_toggle in &extension_to_toggle.locations {
            let expected_library_name = match requires_load.get(&extension_to_toggle.name) {
                None => &extension_to_toggle.name,
                Some(expected_library_name) => expected_library_name,
            };
            // Get extensions trunk project name
            let trunk_project_name =
                get_trunk_project_for_extension(extension_to_toggle.name.clone()).await?;

            // Get appropriate version for trunk project
            let loadable_library_name = match trunk_project_name {
                Some(proj_name) => {
                    let trunk_project_version =
                        get_trunk_project_version(cdb, &proj_name, location_to_toggle).await?;

                    // If version is None, error
                    let Some(trunk_project_version) = trunk_project_version else {
                        error!("Version for {} is none. Version should never be none when toggling an extension", extension_to_toggle.name);
                        continue;
                    };

                    let loadable_library_name = get_loadable_library_name(
                        &proj_name,
                        Version::TrunkProject(&trunk_project_version),
                        &extension_to_toggle.name,
                    )
                    .await?;

                    let control_file_absent = is_control_file_absent(
                        &proj_name,
                        Version::TrunkProject(&trunk_project_version),
                    )
                    .await?;
                    if control_file_absent && loadable_library_name.is_some() {
                        info!(
                            "Extension {} must be enabled with LOAD. Skipping toggle.",
                            extension_to_toggle.name,
                        );
                        continue;
                    }
                    loadable_library_name
                }
                _ => {
                    error!(
                        "Trunk project name for {} is none.",
                        extension_to_toggle.name
                    );
                    None
                }
            };

            // Check if extension has a loadable library

            // If we are toggling on,
            // the extension is included in the REQUIRES_LOAD list,
            // and also is not present in shared_preload_libraries,
            // then requeue.
            if location_to_toggle.enabled
                && (requires_load.contains_key(&extension_to_toggle.name)
                    || loadable_library_name.is_some())
                && !(current_shared_preload_libraries.contains(expected_library_name))
            {
                warn!(
                    "Extension {} requires load, but is not present in shared_preload_libraries for {}, checking if we should requeue.",
                    extension_to_toggle.name, cdb.metadata.name.clone().unwrap());
                // Requeue only if we are expecting a shared preload library that is not yet present
                requeue_if_expecting_shared_preload_library(
                    cdb,
                    &extension_to_toggle.name,
                    requires_load.clone(),
                )?;
            }

            match database_queries::toggle_extension(
                cdb,
                &extension_to_toggle.name,
                location_to_toggle.clone(),
                ctx.clone(),
            )
            .await
            {
                Ok(_) => {}
                Err(ToggleError::WithAction(action)) => {
                    return Err(action);
                }
                Err(ToggleError::WithDescription(error_message)) => {
                    let mut location_status = match types::get_location_status(
                        cdb,
                        &extension_to_toggle.name,
                        &location_to_toggle.database,
                    ) {
                        None => {
                            error!("There should always be an extension status for a location before attempting to toggle an extension for that location");
                            ExtensionInstallLocationStatus {
                                database: location_to_toggle.database.clone(),
                                schema: None,
                                version: None,
                                enabled: None,
                                error: Some(true),
                                error_message: None,
                            }
                        }
                        Some(location_status) => location_status,
                    };
                    location_status.error = Some(true);
                    location_status.error_message = Some(error_message);
                    ext_status_updates = kubernetes_queries::update_extension_location_in_status(
                        cdb,
                        ctx.clone(),
                        &extension_to_toggle.name,
                        &location_status,
                    )
                    .await?;
                }
            }
        }
    }
    Ok(ext_status_updates)
}

// In this function, we check if we are awaiting restart on shared_preload_libraries
fn requeue_if_expecting_shared_preload_library(
    cdb: &CoreDB,
    extension_to_toggle: &str,
    requires_load: BTreeMap<String, String>,
) -> Result<(), Action> {
    let expected_library_name = match requires_load.get(extension_to_toggle) {
        None => extension_to_toggle,
        Some(expected_library_name) => expected_library_name,
    };
    // Get config by name
    match cdb
        .spec
        .get_pg_config_by_name("shared_preload_libraries", requires_load.clone())
    {
        // If there is not an error
        Ok(shared_preload_libraries_config_value) => match shared_preload_libraries_config_value {
            // If there is no value, then we are not expecting a restart
            None => {
                warn!(
                     "Extension {} requires load, but shared_preload_libraries is not configured for {}, so we are not expecting a restart. Continuing.",
                     extension_to_toggle, cdb.metadata.name.clone().unwrap());
            }
            // If there is a value, then we are expecting a restart if the extension name is in the value
            Some(value) => match value.value.to_string().contains(expected_library_name) {
                true => {
                    warn!(
                         "Extension {} requires load, and is present in shared_preload_libraries for {}, requeuing.",
                         extension_to_toggle, cdb.metadata.name.clone().unwrap());
                    return Err(Action::requeue(Duration::from_secs(10)));
                }
                false => {
                    warn!(
                         "Extension {} requires load, but is not present in shared_preload_libraries for {}, allowing error.",
                         extension_to_toggle, cdb.metadata.name.clone().unwrap());
                }
            },
        },
        Err(e) => {
            error!("Error getting shared_preload_libraries config value: {}", e);
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    Ok(())
}

pub fn determine_updated_extensions_status(
    cdb: &CoreDB,
    all_actually_installed_extensions: Vec<ExtensionStatus>,
) -> Vec<ExtensionStatus> {
    // Our results - what we will update the status to
    let mut ext_status_updates: Vec<ExtensionStatus> = vec![];
    // For every actually installed extension
    for actual_extension in all_actually_installed_extensions {
        let mut extension_status = ExtensionStatus {
            name: actual_extension.name.clone(),
            description: actual_extension.description.clone(),
            locations: vec![],
        };
        // For every location of an actually installed extension
        for actual_location in actual_extension.locations {
            // Create a location status
            let mut location_status = ExtensionInstallLocationStatus {
                enabled: actual_location.enabled,
                database: actual_location.database.clone(),
                schema: actual_location.schema.clone(),
                version: actual_location.version.clone(),
                error: Some(false),
                error_message: None,
            };
            // If there is a current status, retain the error and error message if the schema has not changed
            match types::get_location_status(
                cdb,
                &actual_extension.name.clone(),
                &actual_location.database.clone(),
            ) {
                None => {}
                Some(current_status) => {
                    if current_status.schema == actual_location.schema {
                        location_status.error = current_status.error;
                        location_status.error_message = current_status.error_message;
                    }
                }
            }
            // If the desired state matches the actual state, unset the error and error message
            match types::get_location_spec(cdb, &actual_extension.name, &actual_location.database) {
                None => {}
                Some(desired_location) => {
                    if actual_location.enabled == Some(desired_location.enabled) {
                        location_status.error = Some(false);
                        location_status.error_message = None;
                    }
                }
            }
            extension_status.locations.push(location_status);
        }
        // Make unique by database name
        extension_status
            .locations
            .dedup_by(|a, b| a.database == b.database);
        // sort locations by database and schema so the order is deterministic
        extension_status
            .locations
            .sort_by(|a, b| a.database.cmp(&b.database));
        ext_status_updates.push(extension_status);
    }
    let mut cdb_with_updated_extensions_status = cdb.clone();
    cdb_with_updated_extensions_status.status = Some(CoreDBStatus {
        extensions: Some(ext_status_updates.clone()),
        ..CoreDBStatus::default()
    });
    // We also want to include unavailable extensions if they are being requested
    for desired_extension in &cdb.spec.extensions {
        // For every location of the desired extension
        for desired_location in &desired_extension.locations {
            // If the desired location is not in the current status
            // and the desired location is enabled, then
            // we need to add it into the status as unavailable.
            if desired_location.clone().enabled
                && get_location_status(
                    &cdb_with_updated_extensions_status,
                    &desired_extension.name,
                    &desired_location.database,
                )
                .is_none()
            {
                let location_status = ExtensionInstallLocationStatus {
                    enabled: None,
                    database: desired_location.database.clone(),
                    schema: None,
                    version: desired_location.version.clone(),
                    error: Some(true),
                    error_message: Some("Extension is not installed".to_string()),
                };
                ext_status_updates = merge_location_status_into_extension_status_list(
                    &desired_extension.name.clone(),
                    &location_status,
                    ext_status_updates.clone(),
                )
            }
        }
    }
    ext_status_updates.dedup_by(|a, b| a.name == b.name);
    // sort by extension name so the order is deterministic
    ext_status_updates.sort_by(|a, b| a.name.cmp(&b.name));
    ext_status_updates
}

pub fn determine_extension_locations_to_toggle(cdb: &CoreDB) -> Vec<Extension> {
    let mut extensions_to_toggle: Vec<Extension> = vec![];
    for desired_extension in &cdb.spec.extensions {
        info!(
            "Checking if we need to toggle extension {}",
            desired_extension.name
        );
        let mut needs_toggle = false;
        let mut extension_to_toggle = desired_extension.clone();
        extension_to_toggle.locations = vec![];
        for desired_location in &desired_extension.locations {
            match types::get_location_status(
                cdb,
                &desired_extension.name,
                &desired_location.database,
            ) {
                None => {
                    error!("When determining extensions to toggle, there should always be a location status for the desired location, because that should be included by determine_updated_extensions_status.");
                }
                Some(actual_status) => {
                    // If we don't have an error already, the extension exists, and the desired does not match the actual
                    // The actual_status.error should not be None because this only exists on old resource versions
                    // and we update actual_status before calling this function. If we find that for some reason, we just skip.
                    if actual_status.error.is_some()
                        && (!actual_status.error.expect(
                            "We just checked this is not none, so we should be able to unwrap.",
                        ) && actual_status.enabled.is_some()
                            && actual_status.enabled.unwrap() != desired_location.enabled)
                    {
                        needs_toggle = true;
                        extension_to_toggle.locations.push(desired_location.clone());
                    }
                }
            }
        }
        if needs_toggle {
            info!("Adding extension {} to toggle list", desired_extension.name);
            extensions_to_toggle.push(extension_to_toggle);
        }
    }
    extensions_to_toggle
}

// Check for extensions enabled with LOAD. When these extensions are installed, they are not
// present in pg_available_extensions. In order to check if they are installed, we need to check
// for the presence of <extension_name>.so file in the pod.
async fn check_for_extensions_enabled_with_load(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    all_actually_installed_extensions: Vec<ExtensionStatus>,
) -> Result<Vec<ExtensionStatus>, Action> {
    // Get list of trunk installed extensions
    let mut extensions_enabled_with_load: Vec<ExtensionStatus> = vec![];
    let trunk_installed_extensions = cdb.spec.trunk_installs.clone();

    // Get trunk installed extensions that are not in all_actually_installed_extensions
    let mut trunk_installed_extensions_not_in_all_actually_installed_extensions: Vec<TrunkInstall> =
        vec![];
    for trunk_installed_extension in trunk_installed_extensions {
        let mut found = false;
        for actually_installed_extension in &all_actually_installed_extensions {
            if trunk_installed_extension.name == actually_installed_extension.name {
                found = true;
            }
        }
        if !found {
            trunk_installed_extensions_not_in_all_actually_installed_extensions
                .push(trunk_installed_extension);
        }
    }

    // Get all pods so we can check for <extension_name>.so
    let all_pods =
        crate::extensions::install::all_fenced_and_non_fenced_pods(cdb, ctx.clone()).await?;
    for pod in all_pods {
        let pod_name = pod.metadata.name.clone().unwrap();
        // Check for <extension_name>.so files
        for extension in trunk_installed_extensions_not_in_all_actually_installed_extensions.clone()
        {
            let found =
                check_for_so_files(cdb, ctx.clone(), &pod_name, extension.name.clone()).await?;
            // If found, add to extensions_with_load
            if found {
                // Get trunk project description for extension
                let trunk_project_name =
                    get_trunk_project_for_extension(extension.name.clone()).await?;
                let description = get_trunk_project_description(
                    trunk_project_name.as_deref().unwrap(),
                    Version::TrunkProject(extension.version.as_deref().unwrap()),
                )
                .await?;

                let mut extension_status = ExtensionStatus {
                    name: extension.name.clone(),
                    description,
                    locations: vec![],
                };

                let extensions = cdb.spec.extensions.clone();
                let mut found = false;
                for desired_extension in extensions {
                    if desired_extension.name == extension.name {
                        found = true;
                        for desired_location in desired_extension.locations {
                            let location_status = ExtensionInstallLocationStatus {
                                enabled: Some(desired_location.enabled),
                                database: desired_location.database.clone(),
                                schema: desired_location.schema.clone(),
                                version: desired_location.version.clone(),
                                error: Some(false),
                                error_message: None,
                            };
                            extension_status.locations.push(location_status);
                        }
                    }
                }
                if !found {
                    // If trunk installed extension is not in cdb.spec.extensions, we need to set a default location status
                    info!("Trunk installed extension {} is not in cdb.spec.extensions. Setting default location status.", extension.name);
                    let location_status = ExtensionInstallLocationStatus {
                        enabled: Some(false),
                        database: "postgres".to_string(),
                        schema: None,
                        version: None,
                        error: Some(false),
                        error_message: None,
                    };
                    extension_status.locations.push(location_status);
                }
                extensions_enabled_with_load.push(extension_status);
            }
        }
    }
    Ok(extensions_enabled_with_load)
}

// Get trunk project version
async fn get_trunk_project_version(
    cdb: &CoreDB,
    trunk_project_name: &str,
    location_to_toggle: &types::ExtensionInstallLocation,
) -> Result<Option<String>, Action> {
    // Check if version is provided in cdb.spec.trunk_installs
    for trunk_install in &cdb.spec.trunk_installs {
        if trunk_install.name == trunk_project_name {
            if let Some(trunk_install_version) = &trunk_install.version {
                return Ok(Some(trunk_install_version.clone()));
            }
        }
    }

    if let Some(extension_version) = &location_to_toggle.version {
        let maybe_trunk_project = get_trunk_project_metadata_for_version(
            trunk_project_name,
            trunk::Version::Extension(&extension_version),
        )
        .await;

        if let Ok(trunk_project) = maybe_trunk_project {
            return Ok(Some(trunk_project.version));
        }
    }

    // If we still haven't managed to identify the latest Trunk project version,
    // assume the latest version
    let latest_trunk_project_version = get_latest_trunk_project_version(trunk_project_name).await?;

    Ok(Some(latest_trunk_project_version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apis::coredb_types::{CoreDB, CoreDBSpec};

    #[test]
    fn test_determine_updated_extensions_status_empty() {
        let cdb = CoreDB {
            metadata: Default::default(),
            spec: CoreDBSpec {
                extensions: vec![],
                ..Default::default()
            },
            status: None,
        };
        let all_actually_installed_extensions = vec![]; // modify as needed
        let result = determine_updated_extensions_status(&cdb, all_actually_installed_extensions);
        assert_eq!(result, vec![]);
    }
}
