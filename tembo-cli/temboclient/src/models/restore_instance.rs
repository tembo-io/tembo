/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestoreInstance {
    #[serde(
        rename = "app_services",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub app_services: Option<Option<Vec<crate::models::AppType>>>,
    #[serde(
        rename = "connection_pooler",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub connection_pooler: Option<Option<Box<crate::models::ConnectionPooler>>>,
    #[serde(
        rename = "cpu",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub cpu: Option<Option<crate::models::Cpu>>,
    #[serde(
        rename = "environment",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub environment: Option<Option<crate::models::Environment>>,
    #[serde(
        rename = "extra_domains_rw",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub extra_domains_rw: Option<Option<Vec<String>>>,
    #[serde(rename = "instance_name")]
    pub instance_name: String,
    #[serde(
        rename = "memory",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub memory: Option<Option<crate::models::Memory>>,
    #[serde(rename = "restore")]
    pub restore: Box<crate::models::Restore>,
    #[serde(
        rename = "storage",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub storage: Option<Option<crate::models::Storage>>,
}

impl RestoreInstance {
    pub fn new(instance_name: String, restore: crate::models::Restore) -> RestoreInstance {
        RestoreInstance {
            app_services: None,
            connection_pooler: None,
            cpu: None,
            environment: None,
            extra_domains_rw: None,
            instance_name,
            memory: None,
            restore: Box::new(restore),
            storage: None,
        }
    }
}
