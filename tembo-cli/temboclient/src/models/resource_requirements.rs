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
pub struct ResourceRequirements {
    #[serde(rename = "limits", default, with = "::serde_with::rust::double_option", skip_serializing_if = "Option::is_none")]
    pub limits: Option<Option<Box<crate::models::Resource>>>,
    #[serde(rename = "requests", default, with = "::serde_with::rust::double_option", skip_serializing_if = "Option::is_none")]
    pub requests: Option<Option<Box<crate::models::Resource>>>,
}

impl ResourceRequirements {
    pub fn new() -> ResourceRequirements {
        ResourceRequirements {
            limits: None,
            requests: None,
        }
    }
}


