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
pub struct Role {
    /// A valid Role ID. Available Role IDs include 'admin' and 'basic_member'.
    #[serde(rename = "id")]
    pub id: String,
    /// The name of the Role.
    #[serde(rename = "name")]
    pub name: String,
}

impl Role {
    pub fn new(id: String, name: String) -> Role {
        Role {
            id,
            name,
        }
    }
}

