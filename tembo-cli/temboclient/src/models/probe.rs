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
pub struct Probe {
    #[serde(rename = "initialDelaySeconds")]
    pub initial_delay_seconds: i32,
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "port")]
    pub port: String,
}

impl Probe {
    pub fn new(initial_delay_seconds: i32, path: String, port: String) -> Probe {
        Probe {
            initial_delay_seconds,
            path,
            port,
        }
    }
}
