use crate::{
    apis::coredb_types::CoreDB,
    extensions::{
        database_queries, kubernetes_queries, types,
        types::{Extension, ExtensionInstallLocationStatus, ExtensionStatus},
    },
    get_current_coredb_resource, Context,
};
use kube::runtime::controller::Action;

use crate::{
    apis::coredb_types::CoreDBStatus,
    extensions::{
        kubernetes_queries::merge_location_status_into_extension_status_list, types::get_location_status,
    },
};
use std::sync::Arc;
use tracing::error;

pub async fn reconcile_extension_toggle_state(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<ExtensionStatus>, Action> {
    let all_actually_installed_extensions = database_queries::get_all_extensions(cdb, ctx.clone()).await?;
    let ext_status_updates = determine_updated_extensions_status(cdb, all_actually_installed_extensions);
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
    let mut ext_status_updates = ext_status_updates.clone();
    for extension_to_toggle in toggle_these_extensions {
        for location_to_toggle in extension_to_toggle.locations {
            match database_queries::toggle_extension(
                cdb,
                &extension_to_toggle.name,
                location_to_toggle.clone(),
                ctx.clone(),
            )
            .await
            {
                Ok(_) => {}
                Err(error_message) => {
                    let mut location_status = match types::get_location_status(
                        cdb,
                        &extension_to_toggle.name,
                        &location_to_toggle.database,
                        location_to_toggle.schema.clone(),
                    ) {
                        None => {
                            error!("There should always be an extension status for a location before attempting to toggle an extension for that location");
                            ExtensionInstallLocationStatus {
                                database: location_to_toggle.database.clone(),
                                schema: location_to_toggle.schema.clone(),
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

fn determine_updated_extensions_status(
    cdb: &CoreDB,
    all_actually_installed_extensions: Vec<Extension>,
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
                enabled: Some(actual_location.enabled),
                database: actual_location.database.clone(),
                schema: actual_location.schema.clone(),
                version: actual_location.version.clone(),
                error: Some(false),
                error_message: None,
            };
            // If there is a current status, retain the error and error message
            match types::get_location_status(
                cdb,
                &actual_extension.name.clone(),
                &actual_location.database.clone(),
                actual_location.schema.clone(),
            ) {
                None => {}
                Some(current_status) => {
                    location_status.error = current_status.error;
                    location_status.error_message = current_status.error_message;
                }
            }
            // If the desired state matches the actual state, unset the error and error message
            match types::get_location_spec(
                cdb,
                &actual_extension.name,
                &actual_location.database,
                actual_location.schema,
            ) {
                None => {}
                Some(desired_location) => {
                    if actual_location.enabled == desired_location.enabled {
                        location_status.error = Some(false);
                        location_status.error_message = None;
                    }
                }
            }
            extension_status.locations.push(location_status);
        }
        // sort locations by database and schema so the order is deterministic
        extension_status
            .locations
            .sort_by(|a, b| a.database.cmp(&b.database).then(a.schema.cmp(&b.schema)));
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
                    desired_location.schema.clone(),
                )
                .is_none()
            {
                let location_status = ExtensionInstallLocationStatus {
                    enabled: None,
                    database: desired_location.database.clone(),
                    schema: desired_location.schema.clone(),
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
    // sort by extension name so the order is deterministic
    ext_status_updates.sort_by(|a, b| a.name.cmp(&b.name));
    ext_status_updates
}

fn determine_extension_locations_to_toggle(cdb: &CoreDB) -> Vec<Extension> {
    let mut extensions_to_toggle: Vec<Extension> = vec![];
    for desired_extension in &cdb.spec.extensions {
        let mut needs_toggle = false;
        let mut extension_to_toggle = desired_extension.clone();
        extension_to_toggle.locations = vec![];
        for desired_location in &desired_extension.locations {
            match types::get_location_status(
                cdb,
                &desired_extension.name,
                &desired_location.database,
                desired_location.schema.clone(),
            ) {
                None => {
                    error!("When determining extensions to toggle, there should always be a location status for the desired location, because that should be included by determine_updated_extensions_status.");
                }
                Some(actual_status) => {
                    // If we don't have an error already, the extension exists, and the desired does not match the actual
                    // The actual_status.error should not be None because this only exists on old resource versions
                    // and we update actual_status before calling this function. If we find that for some reason, we just skip.
                    if actual_status.error.is_some()
                        && (!actual_status
                            .error
                            .expect("We just checked this is not none, so we should be able to unwrap.")
                            && actual_status.enabled.is_some()
                            && actual_status.enabled.unwrap() != desired_location.enabled)
                    {
                        needs_toggle = true;
                        extension_to_toggle.locations.push(desired_location.clone());
                    }
                }
            }
        }
        if needs_toggle {
            extensions_to_toggle.push(extension_to_toggle);
        }
    }
    extensions_to_toggle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apis::coredb_types::{CoreDB, CoreDBSpec, CoreDBStatus},
        extensions::types::{
            get_location_spec, get_location_status, Extension, ExtensionInstallLocation,
            ExtensionInstallLocationStatus, ExtensionStatus,
        },
    };

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


    #[test]
    fn test_toggle_logic() {
        let desired_extensions = vec![
            Extension {
                name: "ext3".to_string(),
                description: None,
                locations: vec![ExtensionInstallLocation {
                    enabled: true,
                    database: "db1".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                }],
            },
            Extension {
                name: "ext1".to_string(),
                description: None,
                locations: vec![
                // Requesting to enable a currently disabled extension
                ExtensionInstallLocation {
                        enabled: true,
                        database: "db_where_its_available_and_disabled".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                },
                // Requesting to disable a currently enabled extension
                ExtensionInstallLocation {
                    enabled: false,
                    database: "db_where_its_available_and_enabled".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                },
                // Requesting to enable a currently disabled extension that is not currently in status
                ExtensionInstallLocation {
                    enabled: true,
                    database: "db_where_its_available_and_disabled_missing_from_status".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                },
                // Requesting to disable a currently enabled extension that is not currently in status
                ExtensionInstallLocation {
                    enabled: false,
                    database: "db_where_its_available_and_enabled_missing_from_status".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                },
                // This situation is if we toggled an extension to True, but it failed to enable
                // And now we toggle it back to false
                ExtensionInstallLocation {
                    enabled: false,
                    database: "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                },
                // This situation is if we toggled an extension to True, but it failed to enable
                // because it wasn't installed, now we toggle it back to false
                ExtensionInstallLocation {
                    enabled: false,
                    database: "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed_because_missing".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                },
                // Requesting to enable an extension that is not installed
                ExtensionInstallLocation {
                    enabled: true,
                    database: "db_where_its_not_available".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                },
                // Requesting to enable an extension that previously failed to enable
                ExtensionInstallLocation {
                    enabled: true,
                    database: "db_where_enable_failed".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                }
                ],
            },
            Extension {
                name: "ext2".to_string(),
                description: None,
                locations: vec![ExtensionInstallLocation {
                    enabled: false,
                    database: "db1".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                }],
            },
        ];

        let current_status = vec![ExtensionStatus {
            name: "ext1".to_string(),
            description: None,
            locations: vec![
                // Requesting to enable a currently disabled extension
                ExtensionInstallLocationStatus {
                    enabled: Some(false),
                    database: "db_where_its_available_and_disabled".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                    error: Some(false),
                    error_message: None,
                },
                // Requesting to disable a currently enabled extension
                ExtensionInstallLocationStatus {
                    enabled: Some(true),
                    database: "db_where_its_available_and_enabled".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                    error: Some(false),
                    error_message: None,
                },
                ExtensionInstallLocationStatus {
                    enabled: Some(false),
                    database: "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed"
                        .to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                    error: Some(true),
                    error_message: Some("Failed to enable extension".to_string()),
                },
                ExtensionInstallLocationStatus {
                    enabled: None,
                    database:
                        "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed_because_missing"
                            .to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                    error: Some(true),
                    error_message: Some("Extension is not installed".to_string()),
                },
                ExtensionInstallLocationStatus {
                    enabled: Some(false),
                    database: "db_where_enable_failed".to_string(),
                    schema: Some("public".to_string()),
                    version: None,
                    error: Some(true),
                    error_message: Some("Failed to enable extension".to_string()),
                },
            ],
        }];

        let cdb = CoreDB {
            metadata: Default::default(),
            spec: CoreDBSpec {
                extensions: desired_extensions,
                ..Default::default()
            },
            status: Some(CoreDBStatus {
                extensions: Some(current_status),
                ..CoreDBStatus::default()
            }),
        };

        let all_actually_installed_extensions = vec![
            Extension {
                name: "ext1".to_string(),
                description: None,
                locations: vec![
                    ExtensionInstallLocation {
                        enabled: false,
                        database: "db_where_its_available_and_disabled".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                    ExtensionInstallLocation {
                        enabled: true,
                        database: "db_where_its_available_and_enabled".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                    ExtensionInstallLocation {
                        enabled: false,
                        database: "db_where_its_available_and_disabled_missing_from_status".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                    ExtensionInstallLocation {
                        enabled: true,
                        database: "db_where_its_available_and_enabled_missing_from_status".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                    ExtensionInstallLocation {
                        enabled: false,
                        database: "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed"
                            .to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                    ExtensionInstallLocation {
                        enabled: false,
                        database: "db_where_enable_failed".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                ],
            },
            Extension {
                name: "ext2".to_string(),
                description: None,
                locations: vec![
                    ExtensionInstallLocation {
                        enabled: true,
                        database: "db2".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                    ExtensionInstallLocation {
                        enabled: true,
                        database: "db1".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                    },
                ],
            },
        ];

        let result = determine_updated_extensions_status(&cdb, all_actually_installed_extensions);

        // Update the extensions status
        let cdb = CoreDB {
            status: Some(CoreDBStatus {
                extensions: Some(result),
                ..CoreDBStatus::default()
            }),
            ..cdb
        };

        // Check that the current status is updated in the expected way from the provided actually_installed_extensions list
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_its_available_and_disabled",
            Some("public".to_string()),
        )
        .unwrap();
        assert_eq!(location_status.enabled, Some(false));
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_its_available_and_enabled",
            Some("public".to_string()),
        )
        .unwrap();
        assert_eq!(location_status.enabled, Some(true));
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_its_available_and_disabled_missing_from_status",
            Some("public".to_string()),
        )
        .unwrap();
        assert_eq!(location_status.enabled, Some(false));
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_its_available_and_enabled_missing_from_status",
            Some("public".to_string()),
        )
        .unwrap();
        assert_eq!(location_status.enabled, Some(true));
        // Toggling and extension back to false, it should clear the error
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed",
            Some("public".to_string()),
        )
        .unwrap();
        assert_eq!(location_status.enabled, Some(false));
        assert!(!location_status.error.unwrap());
        assert!(location_status.error_message.is_none());
        // Toggling and extension back to false because missing, it should remove from status
        assert!(get_location_status(
            &cdb,
            "ext1",
            "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed_because_missing",
            Some("public".to_string())
        )
        .is_none());
        // It should retain error message when it failed on a previous attempt
        let location_status =
            get_location_status(&cdb, "ext1", "db_where_enable_failed", Some("public".to_string())).unwrap();
        assert_eq!(location_status.enabled, Some(false));
        assert!(location_status.error.unwrap());
        assert!(location_status.error_message.is_some());
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_its_not_available",
            Some("public".to_string()),
        )
        .unwrap();
        assert_eq!(location_status.enabled, None);
        assert!(location_status.error.unwrap());
        assert!(location_status.error_message.is_some());

        let extension_locations_to_toggle = determine_extension_locations_to_toggle(&cdb);
        // We just make this CDB so that we can use our getter function to
        // search through the extension results from determine_extension_locations_to_toggle
        let cdb_spec_check = CoreDB {
            spec: CoreDBSpec {
                extensions: extension_locations_to_toggle,
                ..CoreDBSpec::default()
            },
            ..cdb
        };

        // When available and disabled, requesting to enable, we should try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_available_and_disabled",
            Some("public".to_string()),
        )
        .unwrap();
        assert!(location.enabled);
        // When available and enabled, requesting to disable, we should try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_available_and_enabled",
            Some("public".to_string()),
        )
        .unwrap();
        assert!(!location.enabled);
        // When available and disabled, requesting to enable, we should try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_available_and_disabled_missing_from_status",
            Some("public".to_string()),
        )
        .unwrap();
        assert!(location.enabled);
        // When available and enabled, requesting to disable, we should try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_available_and_enabled_missing_from_status",
            Some("public".to_string()),
        )
        .unwrap();
        assert!(!location.enabled);

        // If we toggled an extension to True, but it failed to enable
        // and then we toggle it back to false, then it does not need a toggle
        // because it's already in the desired state as disabled
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed",
            Some("public".to_string()),
        );
        assert!(location.is_none());
        // If we toggled an extension to True, but it failed to enable because missing
        // and then we toggle it back to false, then it does not need a toggle
        // because it's already in the desired state as disabled
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed_because_missing",
            Some("public".to_string()),
        );
        assert!(location.is_none());
        // If we request to enable an extension that is not installed,
        // we should not try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_not_available",
            Some("public".to_string()),
        );
        assert!(location.is_none());
        // If we request to enable an extension that has previously failed to enable,
        // we should not try to toggle it again
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_enable_failed",
            Some("public".to_string()),
        );
        assert!(location.is_none());
    }
}
