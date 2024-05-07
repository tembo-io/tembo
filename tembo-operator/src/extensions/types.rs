use crate::{apis::coredb_types::CoreDB, defaults, extensions::database_queries::check_input};
use lazy_static::lazy_static;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::warn;
use utoipa::ToSchema;

lazy_static! {
    static ref EXTRA_COMMANDS_TO_ENABLE_EXTENSION: HashMap<String, String> = {
        let mut m = HashMap::new();
        m.insert(
            "pg_cron".to_string(),
            "UPDATE cron.job SET nodename = '';".to_string(),
        );
        m
    };
}

/// TrunkInstall allows installation of extensions from the [pgtrunk](https://pgt.dev)
/// registry.  This list should be a list of extension names and versions that you wish to
/// install at runtime using the pgtrunk API.
///
/// This example will install the pg_stat_statements extension at version 1.10.0.
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///  name: test-db
/// spec:
///   trunk_installs:
///   - name: pg_stat_statements
///     version: 1.10.0
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq, ToSchema)]
pub struct TrunkInstall {
    /// The name of the extension to install. This must be the name of the extension as it
    /// appears in the [pgtrunk](https://pgt.dev) registry.
    pub name: String,

    /// The version of the extension to install. If not specified, the latest version will
    /// be used. (Optional)
    pub version: Option<String>,
}

impl From<TrunkInstallStatus> for TrunkInstall {
    fn from(status: TrunkInstallStatus) -> Self {
        TrunkInstall {
            name: status.name,
            version: status.version,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq, ToSchema)]
pub struct TrunkInstallStatus {
    pub name: String,
    pub version: Option<String>,
    pub error: bool,
    #[serde(default)]
    pub loading: bool,
    pub error_message: Option<String>,
    pub installed_to_pods: Option<Vec<String>>,
}

/// Extension lets you define a list of extensions to enable on the instance. To enable
/// extensions, you must specify the name of the extension and the database, schema, and
/// version to enable it on. If the version is not specified, the latest version will be
/// used.  The extension must also be installed on the instance.  To install
/// extensions, please refer to the `TrunkInstall` resource.
///
/// This example will enable the pg_stat_statements extension on the Postgres database
/// in the public schema.
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db
/// spec:
///   extensions:
///   - name: pg_stat_statements
///     locations:
///     - database: postgres
///       enabled: true
///       schema: public
///       version: 1.10.0
/// ````
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq, ToSchema)]
pub struct Extension {
    /// The name of the extension to enable.
    pub name: String,

    /// A description of the extension. (Optional)
    ///
    /// **Default**: "No description provided"
    #[serde(default = "defaults::default_description")]
    pub description: Option<String>,

    /// A list of locations (databases) to enabled the extension on.
    pub locations: Vec<ExtensionInstallLocation>,
}

impl Default for Extension {
    fn default() -> Self {
        Extension {
            name: "pg_stat_statements".to_owned(),
            description: Some(
                " track planning and execution statistics of all SQL statements executed"
                    .to_owned(),
            ),
            locations: vec![ExtensionInstallLocation::default()],
        }
    }
}

impl From<ExtensionStatus> for Extension {
    fn from(status: ExtensionStatus) -> Self {
        let locations = status
            .locations
            .into_iter()
            .map(ExtensionInstallLocation::from)
            .collect();

        Extension {
            name: status.name,
            description: status.description,
            locations,
        }
    }
}

/// ExtensionInstallLocation lets you specify the database, schema, and version to enable
/// an extension on.
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq, ToSchema)]
pub struct ExtensionInstallLocation {
    /// Enable or disable the extension on this Postgres instance.
    pub enabled: bool,

    /// The database to enable the extension on.
    ///
    /// **Default**: "postgres"
    #[serde(default = "defaults::default_database")]
    pub database: String,

    /// The extension version to install. If not specified, the latest version will be used.
    pub version: Option<String>,

    /// The schema to enable the extension on. (eg: "public")
    pub schema: Option<String>,
}

impl Default for ExtensionInstallLocation {
    fn default() -> Self {
        ExtensionInstallLocation {
            enabled: false,
            database: "postgres".to_string(),
            version: None,
            schema: None,
        }
    }
}

impl From<ExtensionInstallLocationStatus> for ExtensionInstallLocation {
    fn from(status: ExtensionInstallLocationStatus) -> Self {
        ExtensionInstallLocation {
            enabled: status.enabled.unwrap_or(false),
            database: status.database,
            schema: status.schema,
            version: status.version,
        }
    }
}

/// generates the CREATE or DROP EXTENSION command for a given extension
/// handles schema specification in the command
pub fn generate_extension_enable_cmd(
    ext_name: &str,
    ext_loc: &ExtensionInstallLocation,
) -> Result<String, String> {
    let schema_name = ext_loc.schema.to_owned();
    if schema_name.is_some() && !check_input(&schema_name.clone().unwrap()) {
        warn!(
            "Extension.Database.Schema is not formatted properly. Skipping operation. {}",
            schema_name.unwrap()
        );
        return Err("Schema name is not formatted properly".to_string());
    }
    let mut command_suffix: String = "".to_string();
    if EXTRA_COMMANDS_TO_ENABLE_EXTENSION.contains_key(ext_name) {
        command_suffix.clone_from(EXTRA_COMMANDS_TO_ENABLE_EXTENSION.get(ext_name).unwrap());
    }
    // only specify the schema if it provided
    let command = match ext_loc.enabled {
        true => match ext_loc.schema.as_ref() {
            Some(schema) => {
                format!(
                    "CREATE EXTENSION IF NOT EXISTS \"{}\" SCHEMA {} CASCADE;{}",
                    ext_name, schema, command_suffix
                )
            }
            None => format!(
                "CREATE EXTENSION IF NOT EXISTS \"{}\" CASCADE;{}",
                ext_name, command_suffix
            ),
        },
        false => format!("DROP EXTENSION IF EXISTS \"{}\" CASCADE;", ext_name),
    };
    Ok(command)
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq, ToSchema)]
pub struct ExtensionStatus {
    pub name: String,
    #[serde(default = "defaults::default_description")]
    pub description: Option<String>,
    pub locations: Vec<ExtensionInstallLocationStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq, ToSchema)]
pub struct ExtensionInstallLocationStatus {
    #[serde(default = "defaults::default_database")]
    pub database: String,
    pub schema: Option<String>,
    pub version: Option<String>,
    // None means this is not actually installed
    pub enabled: Option<bool>,
    // Optional to handle upgrading existing resources
    pub error: Option<bool>,
    pub error_message: Option<String>,
}

pub fn get_location_status(
    cdb: &CoreDB,
    extension_name: &str,
    location_database: &str,
) -> Option<ExtensionInstallLocationStatus> {
    match &cdb.status {
        None => None,
        Some(status) => match &status.extensions {
            None => None,
            Some(extensions) => {
                for extension in extensions {
                    if extension.name == extension_name {
                        for location in &extension.locations {
                            if location.database == location_database {
                                return Some(location.clone());
                            }
                        }
                        return None;
                    }
                }
                None
            }
        },
    }
}

pub fn get_location_spec(
    cdb: &CoreDB,
    extension_name: &str,
    location_database: &str,
) -> Option<ExtensionInstallLocation> {
    for extension in &cdb.spec.extensions {
        if extension.name == extension_name {
            for location in &extension.locations {
                if location.database == location_database {
                    return Some(location.clone());
                }
            }
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apis::coredb_types::{CoreDB, CoreDBSpec, CoreDBStatus},
        extensions::{
            kubernetes_queries::merge_location_status_into_extension_status_list,
            toggle::{
                determine_extension_locations_to_toggle, determine_updated_extensions_status,
            },
        },
    };

    #[test]
    fn test_get_location_status() {
        let location_database = "postgres";
        let location_schema = Some("public".to_string());
        let extension_name = "extension1";
        let location = ExtensionInstallLocationStatus {
            database: location_database.to_owned(),
            schema: location_schema.to_owned(),
            version: Some("1.9".to_owned()),
            enabled: Some(true),
            error: Some(false),
            error_message: None,
        };
        let cdb = CoreDB {
            metadata: Default::default(),
            spec: Default::default(),
            status: Some(CoreDBStatus {
                extensions: Some(vec![ExtensionStatus {
                    name: extension_name.to_owned(),
                    description: None,
                    locations: vec![location.clone()],
                }]),
                ..CoreDBStatus::default()
            }),
        };

        assert_eq!(
            get_location_status(&cdb, extension_name, location_database),
            Some(location)
        );
    }

    #[test]
    fn test_get_location_spec() {
        let location_database = "postgres";
        let extension_name = "extension1";
        let location = ExtensionInstallLocation {
            enabled: true,
            schema: None,
            database: location_database.to_owned(),
            version: Some("1.9".to_owned()),
        };
        let cdb = CoreDB {
            metadata: Default::default(),
            spec: CoreDBSpec {
                extensions: vec![Extension {
                    name: extension_name.to_owned(),
                    description: None,
                    locations: vec![location.clone()],
                }],
                ..CoreDBSpec::default()
            },
            status: None,
        };

        assert_eq!(
            get_location_spec(&cdb, extension_name, location_database),
            Some(location)
        );
    }

    #[test]
    fn test_generate_extension_enable_cmd() {
        // schema not specified
        let loc1 = ExtensionInstallLocation {
            database: "postgres".to_string(),
            schema: None,
            enabled: true,
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc1);
        assert_eq!(
            cmd.unwrap(),
            "CREATE EXTENSION IF NOT EXISTS \"my_ext\" CASCADE;"
        );

        // schema specified
        let loc2 = ExtensionInstallLocation {
            database: "postgres".to_string(),
            schema: None,
            enabled: true,
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc2);
        assert_eq!(
            cmd.unwrap(),
            "CREATE EXTENSION IF NOT EXISTS \"my_ext\" CASCADE;"
        );

        // drop extension
        let loc2 = ExtensionInstallLocation {
            database: "postgres".to_string(),
            schema: None,
            enabled: false,
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc2);
        assert_eq!(cmd.unwrap(), "DROP EXTENSION IF EXISTS \"my_ext\" CASCADE;");
    }

    #[test]
    fn test_toggle_logic() {
        let desired_extensions = vec![
            Extension {
                name: "ext3".to_string(),
                description: None,
                locations: vec![ExtensionInstallLocation {
                    enabled: true,
                    schema: None,
                    database: "db1".to_string(),
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
                        schema: None,
                        database: "db_where_its_available_and_disabled".to_string(),
                        version: None,
                    },
                    // Requesting to disable a currently enabled extension
                    ExtensionInstallLocation {
                        enabled: false,
                        schema: None,
                        database: "db_where_its_available_and_enabled".to_string(),
                        version: None,
                    },
                    // Requesting to enable a currently disabled extension that is not currently in status
                    ExtensionInstallLocation {
                        enabled: true,
                        schema: None,
                        database: "db_where_its_available_and_disabled_missing_from_status".to_string(),
                        version: None,
                    },
                    // Requesting to disable a currently enabled extension that is not currently in status
                    ExtensionInstallLocation {
                        enabled: false,
                        schema: None,
                        database: "db_where_its_available_and_enabled_missing_from_status".to_string(),
                        version: None,
                    },
                    // This situation is if we toggled an extension to True, but it failed to enable
                    // And now we toggle it back to false
                    ExtensionInstallLocation {
                        enabled: false,
                        schema: None,
                        database: "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed".to_string(),
                        version: None,
                    },
                    // This situation is if we toggled an extension to True, but it failed to enable
                    // because it wasn't installed, now we toggle it back to false
                    ExtensionInstallLocation {
                        enabled: false,
                        schema: None,
                        database: "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed_because_missing".to_string(),
                        version: None,
                    },
                    // Requesting to enable an extension that is not installed
                    ExtensionInstallLocation {
                        enabled: true,
                        schema: None,
                        database: "db_where_its_not_available".to_string(),
                        version: None,
                    },
                    // Requesting to enable an extension that previously failed to enable
                    ExtensionInstallLocation {
                        enabled: true,
                        schema: None,
                        database: "db_where_enable_failed".to_string(),
                        version: None,
                    }
                ],
            },
            Extension {
                name: "ext2".to_string(),
                description: None,
                locations: vec![ExtensionInstallLocation {
                    enabled: false,
                    schema: None,
                    database: "db1".to_string(),
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
            ExtensionStatus {
                name: "ext1".to_string(),
                description: None,
                locations: vec![
                    ExtensionInstallLocationStatus {
                        enabled: Some(false),
                        error: None,
                        database: "db_where_its_available_and_disabled".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
                    },
                    ExtensionInstallLocationStatus {
                        enabled: Some(true),
                        error: None,
                        database: "db_where_its_available_and_enabled".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
                    },
                    ExtensionInstallLocationStatus {
                        enabled: Some(false),
                        error: None,
                        database: "db_where_its_available_and_disabled_missing_from_status"
                            .to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
                    },
                    ExtensionInstallLocationStatus {
                        enabled: Some(true),
                        error: None,
                        database: "db_where_its_available_and_enabled_missing_from_status"
                            .to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
                    },
                    ExtensionInstallLocationStatus {
                        enabled: Some(false),
                        error: None,
                        database:
                            "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed"
                                .to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
                    },
                    ExtensionInstallLocationStatus {
                        enabled: Some(false),
                        error: None,
                        database: "db_where_enable_failed".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
                    },
                ],
            },
            ExtensionStatus {
                name: "ext2".to_string(),
                description: None,
                locations: vec![
                    ExtensionInstallLocationStatus {
                        enabled: Some(true),
                        error: None,
                        database: "db2".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
                    },
                    ExtensionInstallLocationStatus {
                        enabled: Some(true),
                        error: None,
                        database: "db1".to_string(),
                        schema: Some("public".to_string()),
                        version: None,
                        error_message: None,
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
        let location_status =
            get_location_status(&cdb, "ext1", "db_where_its_available_and_disabled").unwrap();
        assert_eq!(location_status.enabled, Some(false));
        let location_status =
            get_location_status(&cdb, "ext1", "db_where_its_available_and_enabled").unwrap();
        assert_eq!(location_status.enabled, Some(true));
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_its_available_and_disabled_missing_from_status",
        )
        .unwrap();
        assert_eq!(location_status.enabled, Some(false));
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_its_available_and_enabled_missing_from_status",
        )
        .unwrap();
        assert_eq!(location_status.enabled, Some(true));
        // Toggling and extension back to false, it should clear the error
        let location_status = get_location_status(
            &cdb,
            "ext1",
            "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed",
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
        )
        .is_none());
        // It should retain error message when it failed on a previous attempt
        let location_status = get_location_status(&cdb, "ext1", "db_where_enable_failed").unwrap();
        assert_eq!(location_status.enabled, Some(false));
        assert!(location_status.error.unwrap());
        assert!(location_status.error_message.is_some());
        let location_status =
            get_location_status(&cdb, "ext1", "db_where_its_not_available").unwrap();
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
        )
        .unwrap();
        assert!(location.enabled);
        // When available and enabled, requesting to disable, we should try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_available_and_enabled",
        )
        .unwrap();
        assert!(!location.enabled);
        // When available and disabled, requesting to enable, we should try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_available_and_disabled_missing_from_status",
        )
        .unwrap();
        assert!(location.enabled);
        // When available and enabled, requesting to disable, we should try to toggle it
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_its_available_and_enabled_missing_from_status",
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
        );
        assert!(location.is_none());
        // If we toggled an extension to True, but it failed to enable because missing
        // and then we toggle it back to false, then it does not need a toggle
        // because it's already in the desired state as disabled
        let location = get_location_spec(
            &cdb_spec_check,
            "ext1",
            "db_where_it_is_currently_in_error_having_tried_to_enable_and_failed_because_missing",
        );
        assert!(location.is_none());
        // If we request to enable an extension that is not installed,
        // we should not try to toggle it
        let location = get_location_spec(&cdb_spec_check, "ext1", "db_where_its_not_available");
        assert!(location.is_none());
        // If we request to enable an extension that has previously failed to enable,
        // we should not try to toggle it again
        let location = get_location_spec(&cdb_spec_check, "ext1", "db_where_enable_failed");
        assert!(location.is_none());
    }

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
                database: "db2".to_string(),
                schema: Some("schema2".to_string()),
                version: None,
                error: Some(false),
                error_message: None,
            }],
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
        assert_eq!(result[0].locations[1].database, "db2".to_string());
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
    #[test]
    fn test_extension_conversion() {
        let status = ExtensionStatus {
            name: "pgmq".to_string(),
            description: Some("description".to_string()),
            locations: vec![ExtensionInstallLocationStatus {
                database: "postgres".to_string(),
                schema: Some("schema".to_string()),
                version: Some("1.0".to_string()),
                enabled: Some(true),
                error: Some(false),
                error_message: None,
            }],
        };
        let extension: Extension = status.into();
        assert_eq!(extension.name, "pgmq");
        assert_eq!(extension.description, Some("description".to_string()));
        assert_eq!(extension.locations[0].database, "postgres");
        assert_eq!(extension.locations[0].schema, Some("schema".to_string()));
    }

    #[test]
    fn test_trunk_install_conversion() {
        let status = TrunkInstallStatus {
            name: "pgmq".to_string(),
            version: Some("1.0".to_string()),
            error: false,
            error_message: None,
            installed_to_pods: None,
            loading: false,
        };
        let trunk_install: TrunkInstall = status.into();
        assert_eq!(trunk_install.name, "pgmq");
        assert_eq!(trunk_install.version, Some("1.0".to_string()));
    }
}
