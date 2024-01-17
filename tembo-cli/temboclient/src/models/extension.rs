/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

/// Extension : Extension lets you define a list of extensions to enable on the instance. To enable extensions, you must specify the name of the extension and the database, schema, and version to enable it on. If the version is not specified, the latest version will be used.  The extension must also be installed on the instance.  To install extensions, please refer to the `TrunkInstall` resource.  This example will enable the pg_stat_statements extension on the Postgres database in the public schema.  ```yaml apiVersion: coredb.io/v1alpha1 kind: CoreDB metadata: name: test-db spec: extensions: - name: pg_stat_statements locations: - database: postgres enabled: true schema: public version: 1.10.0 ````

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extension {
    /// A description of the extension. (Optional)  **Default**: \"No description provided\"
    #[serde(
        rename = "description",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Option<String>>,
    /// A list of locations (databases) to enabled the extension on.
    #[serde(rename = "locations")]
    pub locations: Vec<crate::models::ExtensionInstallLocation>,
    /// The name of the extension to enable.
    #[serde(rename = "name")]
    pub name: String,
}

impl Extension {
    /// Extension lets you define a list of extensions to enable on the instance. To enable extensions, you must specify the name of the extension and the database, schema, and version to enable it on. If the version is not specified, the latest version will be used.  The extension must also be installed on the instance.  To install extensions, please refer to the `TrunkInstall` resource.  This example will enable the pg_stat_statements extension on the Postgres database in the public schema.  ```yaml apiVersion: coredb.io/v1alpha1 kind: CoreDB metadata: name: test-db spec: extensions: - name: pg_stat_statements locations: - database: postgres enabled: true schema: public version: 1.10.0 ````
    pub fn new(locations: Vec<crate::models::ExtensionInstallLocation>, name: String) -> Extension {
        Extension {
            description: None,
            locations,
            name,
        }
    }
}
