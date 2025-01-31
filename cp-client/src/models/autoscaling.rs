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
pub struct Autoscaling {
    #[serde(rename = "autostop", skip_serializing_if = "Option::is_none")]
    pub autostop: Option<Box<models::AutoStop>>,
    #[serde(rename = "storage", skip_serializing_if = "Option::is_none")]
    pub storage: Option<Box<models::AutoscalingStorage>>,
}

impl Autoscaling {
    pub fn new() -> Autoscaling {
        Autoscaling {
            autostop: None,
            storage: None,
        }
    }
}

