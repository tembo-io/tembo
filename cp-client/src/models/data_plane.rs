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
pub struct DataPlane {
    #[serde(rename = "index")]
    pub index: String,
    #[serde(rename = "provider_id")]
    pub provider_id: String,
    #[serde(rename = "provider_name")]
    pub provider_name: String,
    #[serde(rename = "region")]
    pub region: String,
    #[serde(rename = "region_id")]
    pub region_id: String,
    #[serde(rename = "region_name")]
    pub region_name: String,
}

impl DataPlane {
    pub fn new(
        index: String,
        provider_id: String,
        provider_name: String,
        region: String,
        region_id: String,
        region_name: String,
    ) -> DataPlane {
        DataPlane {
            index,
            provider_id,
            provider_name,
            region,
            region_id,
            region_name,
        }
    }
}
