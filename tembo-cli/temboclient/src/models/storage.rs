/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Storage {
    #[serde(rename = "10Gi")]
    Variant10Gi,
    #[serde(rename = "50Gi")]
    Variant50Gi,
    #[serde(rename = "100Gi")]
    Variant100Gi,
    #[serde(rename = "200Gi")]
    Variant200Gi,
    #[serde(rename = "300Gi")]
    Variant300Gi,
    #[serde(rename = "400Gi")]
    Variant400Gi,
    #[serde(rename = "500Gi")]
    Variant500Gi,
}

impl ToString for Storage {
    fn to_string(&self) -> String {
        match self {
            Self::Variant10Gi => String::from("10Gi"),
            Self::Variant50Gi => String::from("50Gi"),
            Self::Variant100Gi => String::from("100Gi"),
            Self::Variant200Gi => String::from("200Gi"),
            Self::Variant300Gi => String::from("300Gi"),
            Self::Variant400Gi => String::from("400Gi"),
            Self::Variant500Gi => String::from("500Gi"),
        }
    }
}

impl Default for Storage {
    fn default() -> Storage {
        Self::Variant10Gi
    }
}
