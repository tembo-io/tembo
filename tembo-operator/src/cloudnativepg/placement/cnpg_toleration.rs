use crate::cloudnativepg::clusters::ClusterAffinityTolerations;
use crate::cloudnativepg::poolers::PoolerTemplateSpecTolerations;
use k8s_openapi::api::core::v1::Toleration;

// convert_toleration converts a ClusterAffinityTolerations to a k8s_openapi::api::core::v1::Toleration
pub fn convert_toleration(cat: &ClusterAffinityTolerations) -> Toleration {
    Toleration {
        effect: cat.effect.clone(),
        key: cat.key.clone(),
        operator: cat.operator.clone(),
        toleration_seconds: cat.toleration_seconds,
        value: cat.value.clone(),
    }
}

// convert_toleration converts a k8s_openapi::api::core::v1::Toleration to a PoolerTemplateSpecTolerations to
// be used in the PoolerTemplateSpec struct when building out a pooler.
pub fn convert_toleration_to_pooler(
    toleration: &Toleration,
) -> Option<PoolerTemplateSpecTolerations> {
    if toleration.key.is_none() && toleration.effect.is_none() {
        None
    } else {
        Some(PoolerTemplateSpecTolerations {
            effect: toleration.effect.clone(),
            key: toleration.key.clone(),
            operator: toleration.operator.clone().or(Some("Equal".to_string())),
            toleration_seconds: toleration.toleration_seconds,
            value: toleration.value.clone(),
        })
    }
}
