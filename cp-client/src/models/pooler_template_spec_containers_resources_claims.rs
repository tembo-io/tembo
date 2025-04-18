/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

use serde::{Deserialize, Serialize};

/// PoolerTemplateSpecContainersResourcesClaims : ResourceClaim references one entry in PodSpec.ResourceClaims.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoolerTemplateSpecContainersResourcesClaims {
    /// Name must match the name of one entry in pod.spec.resourceClaims of the Pod where this field is used. It makes that resource available inside a container.
    #[serde(rename = "name")]
    pub name: String,
}

impl PoolerTemplateSpecContainersResourcesClaims {
    /// ResourceClaim references one entry in PodSpec.ResourceClaims.
    pub fn new(name: String) -> PoolerTemplateSpecContainersResourcesClaims {
        PoolerTemplateSpecContainersResourcesClaims { name }
    }
}
