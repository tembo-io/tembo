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
pub struct AppTypeOneOf4 {
    #[serde(rename = "pganalyze", deserialize_with = "Option::deserialize")]
    pub pganalyze: Option<Box<crate::models::AppConfig>>,
}

impl AppTypeOneOf4 {
    pub fn new(pganalyze: Option<crate::models::AppConfig>) -> AppTypeOneOf4 {
        AppTypeOneOf4 {
            pganalyze: if let Some(x) = pganalyze {Some(Box::new(x))} else {None},
        }
    }
}


