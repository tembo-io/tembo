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

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::Toleration;

    #[test]
    fn test_convert_toleration() {
        let cat = ClusterAffinityTolerations {
            effect: Some("NoExecute".to_string()),
            key: Some("key1".to_string()),
            operator: Some("Exists".to_string()),
            toleration_seconds: Some(3600),
            value: Some("value1".to_string()),
        };

        let result = convert_toleration(&cat);
        assert_eq!(result.effect, Some("NoExecute".to_string()));
        assert_eq!(result.key, Some("key1".to_string()));
        assert_eq!(result.operator, Some("Exists".to_string()));
        assert_eq!(result.toleration_seconds, Some(3600));
        assert_eq!(result.value, Some("value1".to_string()));
    }

    #[test]
    fn test_convert_toleration_to_pooler_non_empty_key_and_effect() {
        let toleration = Toleration {
            effect: Some("NoExecute".to_string()),
            key: Some("key1".to_string()),
            operator: None,
            toleration_seconds: Some(3600),
            value: Some("value1".to_string()),
        };

        let result = convert_toleration_to_pooler(&toleration);
        assert!(result.is_some());
        let pooler_toleration = result.unwrap();
        assert_eq!(pooler_toleration.effect, Some("NoExecute".to_string()));
        assert_eq!(pooler_toleration.key, Some("key1".to_string()));
        assert_eq!(pooler_toleration.operator, Some("Equal".to_string())); // Default operator
        assert_eq!(pooler_toleration.toleration_seconds, Some(3600));
        assert_eq!(pooler_toleration.value, Some("value1".to_string()));
    }

    #[test]
    fn test_convert_toleration_to_pooler_empty_key_and_effect() {
        let toleration = Toleration {
            effect: None,
            key: None,
            operator: None,
            toleration_seconds: None,
            value: None,
        };

        let result = convert_toleration_to_pooler(&toleration);
        assert!(result.is_none());
    }
}
