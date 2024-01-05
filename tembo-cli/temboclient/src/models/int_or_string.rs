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
pub struct IntOrString {
    #[serde(rename = "Int")]
    pub int: i32,
    #[serde(rename = "String")]
    pub string: String,
}

impl IntOrString {
    pub fn new(int: i32, string: String) -> IntOrString {
        IntOrString { int, string }
    }
}
