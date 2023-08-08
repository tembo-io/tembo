use crate::{apis::coredb_types::CoreDB, defaults};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct TrunkInstall {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct TrunkInstallStatus {
    pub name: String,
    pub version: Option<String>,
    pub status: InstallStatus,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub enum InstallStatus {
    Installed,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct Extension {
    pub name: String,
    #[serde(default = "defaults::default_description")]
    pub description: Option<String>,
    pub locations: Vec<ExtensionInstallLocation>,
}

impl Default for Extension {
    fn default() -> Self {
        Extension {
            name: "pg_stat_statements".to_owned(),
            description: Some(
                " track planning and execution statistics of all SQL statements executed".to_owned(),
            ),
            locations: vec![ExtensionInstallLocation::default()],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct ExtensionInstallLocation {
    pub enabled: bool,
    // no database or schema when disabled
    #[serde(default = "defaults::default_database")]
    pub database: String,
    #[serde(default = "defaults::default_schema")]
    pub schema: String,
    pub version: Option<String>,
}

impl Default for ExtensionInstallLocation {
    fn default() -> Self {
        ExtensionInstallLocation {
            schema: "public".to_owned(),
            database: "postgres".to_owned(),
            enabled: true,
            version: Some("1.9".to_owned()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct ExtensionStatus {
    pub name: String,
    #[serde(default = "defaults::default_description")]
    pub description: Option<String>,
    pub locations: Vec<ExtensionInstallLocationStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct ExtensionInstallLocationStatus {
    #[serde(default = "defaults::default_database")]
    pub database: String,
    #[serde(default = "defaults::default_schema")]
    pub schema: String,
    pub version: Option<String>,
    // None means this is not actually installed
    pub enabled: Option<bool>,
    pub error: bool,
    pub error_message: Option<String>,
}

pub fn get_location_status(
    cdb: &CoreDB,
    extension_name: &str,
    location_database: &str,
    location_schema: &str,
) -> Option<ExtensionInstallLocationStatus> {
    match &cdb.status {
        None => None,
        Some(status) => match &status.extensions {
            None => None,
            Some(extensions) => {
                for extension in extensions {
                    if extension.name == extension_name {
                        for location in &extension.locations {
                            if location.database == location_database && location.schema == location_schema {
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
    location_schema: &str,
) -> Option<ExtensionInstallLocation> {
    for extension in &cdb.spec.extensions {
        if extension.name == extension_name {
            for location in &extension.locations {
                if location.database == location_database && location.schema == location_schema {
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
    use crate::apis::coredb_types::{CoreDB, CoreDBSpec, CoreDBStatus};

    #[test]
    fn test_get_location_status() {
        let location_database = "postgres";
        let location_schema = "public";
        let extension_name = "extension1";
        let location = ExtensionInstallLocationStatus {
            database: location_database.to_owned(),
            schema: location_schema.to_owned(),
            version: Some("1.9".to_owned()),
            enabled: Some(true),
            error: false,
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
            get_location_status(&cdb, extension_name, location_database, location_schema),
            Some(location)
        );
    }

    #[test]
    fn test_get_location_spec() {
        let location_database = "postgres";
        let location_schema = "public";
        let extension_name = "extension1";
        let location = ExtensionInstallLocation {
            enabled: true,
            database: location_database.to_owned(),
            schema: location_schema.to_owned(),
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
            get_location_spec(&cdb, extension_name, location_database, location_schema),
            Some(location)
        );
    }
}
