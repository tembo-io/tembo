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
pub struct AppType {
    #[serde(rename = "restapi", deserialize_with = "Option::deserialize")]
    pub restapi: Option<Box<crate::models::AppConfig>>,
    #[serde(rename = "http", deserialize_with = "Option::deserialize")]
    pub http: Option<Box<crate::models::AppConfig>>,
    #[serde(rename = "mq-api", deserialize_with = "Option::deserialize")]
    pub mq_api: Option<Box<crate::models::AppConfig>>,
    #[serde(rename = "embeddings", deserialize_with = "Option::deserialize")]
    pub embeddings: Option<Box<crate::models::AppConfig>>,
    #[serde(rename = "pganalyze", deserialize_with = "Option::deserialize")]
    pub pganalyze: Option<Box<crate::models::AppConfig>>,
    #[serde(rename = "custom")]
    pub custom: Option<Box<crate::models::AppService>>,
}

impl AppType {
    pub fn new(restapi: Option<crate::models::AppConfig>, http: Option<crate::models::AppConfig>, mq_api: Option<crate::models::AppConfig>, embeddings: Option<crate::models::AppConfig>, pganalyze: Option<crate::models::AppConfig>, custom: Option<crate::models::AppService>) -> AppType {
        AppType {
            restapi: if let Some(x) = restapi {Some(Box::new(x))} else {None},
            http: if let Some(x) = http {Some(Box::new(x))} else {None},
            mq_api: if let Some(x) = mq_api {Some(Box::new(x))} else {None},
            embeddings: if let Some(x) = embeddings {Some(Box::new(x))} else {None},
            pganalyze: if let Some(x) = pganalyze {Some(Box::new(x))} else {None},
            custom: if let Some(x) = custom {Some(Box::new(x))} else {None},
        }
    }
}


