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
pub struct TrunkInstallStatus {
    #[serde(rename = "error")]
    pub error: bool,
    #[serde(
        rename = "error_message",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub error_message: Option<Option<String>>,
    #[serde(
        rename = "installed_to_pods",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub installed_to_pods: Option<Option<Vec<String>>>,
    #[serde(rename = "loading", skip_serializing_if = "Option::is_none")]
    pub loading: Option<bool>,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(
        rename = "version",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Option<Option<String>>,
}

impl TrunkInstallStatus {
    pub fn new(error: bool, name: String) -> TrunkInstallStatus {
        TrunkInstallStatus {
            error,
            error_message: None,
            installed_to_pods: None,
            loading: None,
            name,
            version: None,
        }
    }
}
