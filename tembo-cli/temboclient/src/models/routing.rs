/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)             
 *
 * The version of the OpenAPI document: v1.0.0
 * 
 * Generated by: https://openapi-generator.tech
 */

/// Routing : Routing is used if there is a routing port, then a service is created using that Port when ingress_path is present, an ingress is created. Otherwise, no ingress is created



#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Routing {
    #[serde(rename = "entryPoints", default, with = "::serde_with::rust::double_option", skip_serializing_if = "Option::is_none")]
    pub entry_points: Option<Option<Vec<String>>>,
    #[serde(rename = "ingressPath", default, with = "::serde_with::rust::double_option", skip_serializing_if = "Option::is_none")]
    pub ingress_path: Option<Option<String>>,
    #[serde(rename = "ingressType", default, with = "::serde_with::rust::double_option", skip_serializing_if = "Option::is_none")]
    pub ingress_type: Option<Option<crate::models::IngressType>>,
    /// provide name of the middleware resources to apply to this route
    #[serde(rename = "middlewares", default, with = "::serde_with::rust::double_option", skip_serializing_if = "Option::is_none")]
    pub middlewares: Option<Option<Vec<String>>>,
    #[serde(rename = "port")]
    pub port: i32,
}

impl Routing {
    /// Routing is used if there is a routing port, then a service is created using that Port when ingress_path is present, an ingress is created. Otherwise, no ingress is created
    pub fn new(port: i32) -> Routing {
        Routing {
            entry_points: None,
            ingress_path: None,
            ingress_type: None,
            middlewares: None,
            port,
        }
    }
}


