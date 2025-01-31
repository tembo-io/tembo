/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(
        rename = "env",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub env: Option<Option<Vec<models::EnvVar>>>,
    #[serde(
        rename = "resources",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub resources: Option<Option<Box<models::ResourceRequirements>>>,
}

impl AppConfig {
    pub fn new() -> AppConfig {
        AppConfig {
            env: None,
            resources: None,
        }
    }
}
