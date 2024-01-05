/*
 * Tembo Data API
 *
 * In the case of large or sensitive data, we avoid collecting it into Tembo Cloud. Instead, there is a Tembo Data API for each region, cloud, or private data plane.             </br>             </br>             To find the Tembo Cloud API, please find it [here](https://api.tembo.io/swagger-ui/).
 *
 * The version of the OpenAPI document: v0.0.1
 *
 * Generated by: https://openapi-generator.tech
 */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvailableSecret {
    /// The name of an available secret
    #[serde(rename = "name")]
    pub name: String,
    /// For this secret, available keys
    #[serde(rename = "possible_keys")]
    pub possible_keys: Vec<String>,
}

impl AvailableSecret {
    pub fn new(name: String, possible_keys: Vec<String>) -> AvailableSecret {
        AvailableSecret {
            name,
            possible_keys,
        }
    }
}
