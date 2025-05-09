/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

use serde::{Deserialize, Serialize};

/// ExtensionInstallLocation : ExtensionInstallLocation lets you specify the database, schema, and version to enable an extension on.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInstallLocation {
    /// The database to enable the extension on.  **Default**: \"postgres\"
    #[serde(rename = "database", skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Enable or disable the extension on this Postgres instance.
    #[serde(rename = "enabled")]
    pub enabled: bool,
    /// The schema to enable the extension on. (eg: \"public\")
    #[serde(
        rename = "schema",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub schema: Option<Option<String>>,
    /// The extension version to install. If not specified, the latest version will be used.
    #[serde(
        rename = "version",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Option<Option<String>>,
}

impl ExtensionInstallLocation {
    /// ExtensionInstallLocation lets you specify the database, schema, and version to enable an extension on.
    pub fn new(enabled: bool) -> ExtensionInstallLocation {
        ExtensionInstallLocation {
            database: None,
            enabled,
            schema: None,
            version: None,
        }
    }
}
