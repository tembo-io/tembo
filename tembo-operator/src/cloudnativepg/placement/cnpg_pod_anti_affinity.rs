use crate::cloudnativepg::clusters::{
    ClusterAffinityAdditionalPodAntiAffinity,
    ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
    ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
    ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
    ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution,
    ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
    ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
};
use crate::cloudnativepg::poolers::{
    PoolerTemplateSpecAffinityPodAntiAffinity,
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecution,
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions,
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelectorMatchExpressions,
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution,
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelectorMatchExpressions,
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelectorMatchExpressions,
};
use k8s_openapi::api::core::v1::{PodAffinityTerm, PodAntiAffinity, WeightedPodAffinityTerm};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement};

// Start of helper functions to convert a ClusterAffinityPodAntiAffinity to a k8s_openapi::api::core::v1::PodAntiAffinity
//
// convert_pod_anti_affinity converts a ClusterAffinityAdditionalPodAntiAffinity to a PodAntiAffinity
pub fn convert_pod_anti_affinity(
    cluster_affinity: &ClusterAffinityAdditionalPodAntiAffinity,
) -> PodAntiAffinity {
    PodAntiAffinity {
        required_during_scheduling_ignored_during_execution: cluster_affinity
            .required_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|req| {
                req.iter()
                    .map(convert_required_pod_anti_affinity_term)
                    .collect()
            }),
        preferred_during_scheduling_ignored_during_execution: cluster_affinity
            .preferred_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|prefs| {
                prefs
                    .iter()
                    .map(|pref| WeightedPodAffinityTerm {
                        weight: pref.weight,
                        pod_affinity_term: convert_exec_pod_anti_affinity_term(
                            &pref.pod_affinity_term,
                        ),
                    })
                    .collect()
            }),
    }
}

// convert_required_pod_anti_affinity_term converts a ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution to a PodAffinityTerm
fn convert_required_pod_anti_affinity_term(
    term: &ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution,
) -> PodAffinityTerm {
    PodAffinityTerm {
        label_selector: term
            .label_selector
            .as_ref()
            .map(convert_anti_affinity_exec_label_selector),
        namespace_selector: term
            .namespace_selector
            .as_ref()
            .map(convert_anti_affinity_exec_namespace_selector),
        namespaces: term.namespaces.clone(),
        topology_key: term.topology_key.clone(),
        ..Default::default()
    }
}

// convert_anti_affinity_exec_label_selector converts a ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector to a LabelSelector
fn convert_anti_affinity_exec_label_selector(
    selector: &ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
) -> LabelSelector {
    LabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|expressions| {
            expressions
                .iter()
                .map(|expr| LabelSelectorRequirement {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: Some(
                        expr.values
                            .as_ref()
                            .map_or_else(Vec::new, |vals| vals.clone()),
                    ),
                })
                .collect()
        }),
    }
}

// convert_anti_affinity_exec_namespace_selector converts a ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector to a LabelSelector
fn convert_anti_affinity_exec_namespace_selector(
    ns_selector: &ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
) -> LabelSelector {
    LabelSelector {
        match_labels: ns_selector.match_labels.clone(),
        match_expressions: ns_selector.match_expressions.as_ref().map(|exprs| {
            exprs
                .iter()
                .map(|expr| LabelSelectorRequirement {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: Some(
                        expr.values
                            .as_ref()
                            .map_or_else(Vec::new, |vals| vals.clone()),
                    ),
                })
                .collect()
        }),
    }
}

// convert_exec_pod_anti_affinity_term converts a ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm to a PodAffinityTerm
fn convert_exec_pod_anti_affinity_term(
    term: &ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
) -> PodAffinityTerm {
    PodAffinityTerm {
        label_selector: term
            .label_selector
            .as_ref()
            .map(convert_anti_affinity_term_label_selector),
        namespace_selector: term
            .namespace_selector
            .as_ref()
            .map(convert_anti_affinity_term_namespace_selector),
        namespaces: term.namespaces.clone(),
        topology_key: term.topology_key.clone(),
        ..Default::default()
    }
}

// convert_anti_affinity_term_namespace_selector converts a ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector to a LabelSelector
fn convert_anti_affinity_term_namespace_selector(
    ns_selector: &ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
) -> LabelSelector {
    // Conversion logic here, assuming it's the same as convert_label_selector logic
    LabelSelector {
        match_labels: ns_selector.match_labels.clone(),
        match_expressions: ns_selector.match_expressions.as_ref().map(|exprs| {
            exprs
                .iter()
                .map(|expr| LabelSelectorRequirement {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: Some(
                        expr.values
                            .as_ref()
                            .map_or_else(Vec::new, |vals| vals.clone()),
                    ),
                })
                .collect()
        }),
    }
}

// convert_anti_affinity_term_label_selector converts a ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector to a LabelSelector
fn convert_anti_affinity_term_label_selector(
    selector: &ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
) -> LabelSelector {
    LabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|expressions| {
            expressions
                .iter()
                .map(|expr| LabelSelectorRequirement {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: Some(
                        expr.values
                            .as_ref()
                            .map_or_else(Vec::new, |vals| vals.clone()),
                    ),
                })
                .collect()
        }),
    }
}

// Start functions that do conversions to the PoolerTemplateSpec structs
//
// convert_pod_anti_affinity_to_pooler converts a PodAntiAffinity to a PoolerTemplateSpecAffinityPodAntiAffinity struct
// to be used in the PoolerTemplateSpec struct when building out a pooler.
pub fn convert_pod_anti_affinity_to_pooler(
    pod_affinity: &PodAntiAffinity,
) -> Option<PoolerTemplateSpecAffinityPodAntiAffinity> {
    Some(PoolerTemplateSpecAffinityPodAntiAffinity {
        required_during_scheduling_ignored_during_execution: pod_affinity
            .required_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|req| {
                req.iter()
                    .map(convert_required_pod_anti_affinity_term_to_pooler)
                    .collect()
            }),
        preferred_during_scheduling_ignored_during_execution: pod_affinity
            .preferred_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|prefs| {
                prefs
                    .iter()
                    .map(|pref| PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecution {
                        pod_affinity_term: convert_preferred_pod_anti_affinity_term_to_pooler(&pref.pod_affinity_term),
                        weight: pref.weight,
                    })
                    .collect()
            }),
    })
}

// convert_required_pod_anti_affinity_term_to_pooler converts a PodAffinityTerm to a PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution
// to be used in the PoolerTemplateSpecAffinityPodAntiAffinity struct when building out a pooler.
fn convert_required_pod_anti_affinity_term_to_pooler(
    term: &PodAffinityTerm,
) -> PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution {
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution {
        label_selector: term
            .label_selector
            .as_ref()
            .map(convert_required_label_selector_to_pooler),
        namespace_selector: term
            .namespace_selector
            .as_ref()
            .map(convert_required_namespace_selector_to_pooler),
        namespaces: term.namespaces.clone(),
        topology_key: term.topology_key.clone(),
    }
}

// convert_preferred_pod_anti_affinity_term_to_pooler converts a PodAffinityTerm to a PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm
fn convert_preferred_pod_anti_affinity_term_to_pooler(
    term: &PodAffinityTerm,
) -> PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm{
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm {
        label_selector: term
            .label_selector
            .as_ref()
            .map(convert_preferred_label_selector_to_pooler),
        namespace_selector: term
            .namespace_selector
            .as_ref()
            .map(convert_preferred_namespace_selector_to_pooler),
        namespaces: term.namespaces.clone(),
        topology_key: term.topology_key.clone(),
    }
}

// convert_preferred_namespace_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector
fn convert_preferred_namespace_selector_to_pooler(
    ns_selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector{
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector {
        match_labels: ns_selector.match_labels.clone(),
        match_expressions: ns_selector.match_expressions.as_ref().map(|exprs| {
            exprs.iter().map(|expr| {
                PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelectorMatchExpressions {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: expr.values.clone(),
                }
            }).collect()
        }),
    }
}

// convert_preferred_label_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector
fn convert_preferred_label_selector_to_pooler(
    selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector{
    PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|exprs| exprs.iter().map(|expr| {
            PoolerTemplateSpecAffinityPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions {
                key: expr.key.clone(),
                operator: expr.operator.clone(),
                values: expr.values.clone(),
            }
        }).collect()),
    }
}

// convert_required_namespace_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector
fn convert_required_namespace_selector_to_pooler(
    ns_selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector{
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector {
        match_labels: ns_selector.match_labels.clone(),
        match_expressions: ns_selector.match_expressions.as_ref().map(|exprs| {
            exprs.iter().map(|expr| {
                PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelectorMatchExpressions {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: expr.values.clone(),
                }
            }).collect()
        }),
    }
}

// convert_required_label_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector
fn convert_required_label_selector_to_pooler(
    selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector
{
    PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|exprs| {
            exprs.iter().map(|expr| {
                PoolerTemplateSpecAffinityPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelectorMatchExpressions {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: expr.values.clone(),
                }
            }).collect()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloudnativepg::clusters::ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions;

    fn create_sample_pod_anti_affinity() -> PodAntiAffinity {
        PodAntiAffinity {
            required_during_scheduling_ignored_during_execution: Some(vec![PodAffinityTerm {
                label_selector: Some(LabelSelector {
                    match_labels: Some(
                        [("key1", "value1")]
                            .iter()
                            .cloned()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect(),
                    ),
                    ..Default::default()
                }),
                namespaces: Some(vec!["default".to_string()]),
                topology_key: "kubernetes.io/hostname".to_string(),
                ..Default::default()
            }]),
            preferred_during_scheduling_ignored_during_execution: Some(vec![
                WeightedPodAffinityTerm {
                    weight: 100,
                    pod_affinity_term: PodAffinityTerm {
                        label_selector: Some(LabelSelector {
                            match_labels: Some(
                                [("key2", "value2")]
                                    .iter()
                                    .cloned()
                                    .map(|(k, v)| (k.to_string(), v.to_string()))
                                    .collect(),
                            ),
                            ..Default::default()
                        }),
                        namespaces: Some(vec!["default".to_string()]),
                        topology_key: "kubernetes.io/zone".to_string(),
                        ..Default::default()
                    },
                },
            ]),
        }
    }

    #[test]
    fn test_convert_pod_anti_affinity_term_full() {
        let term = ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm {
            label_selector: Some(ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector {
                match_labels: Some(std::collections::BTreeMap::from([
                    ("key1".to_string(), "value1".to_string()),
                    ("key2".to_string(), "value2".to_string()),
                ])),
                match_expressions: Some(vec![
                    ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions {
                        key: "exp_key".to_string(),
                        operator: "In".to_string(),
                        values: Some(vec!["val1".to_string(), "val2".to_string()]),
                    },
                ]),
            }),
            namespace_selector: Some(ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector {
                match_labels: Some(std::collections::BTreeMap::from([("ns_key".to_string(), "ns_value".to_string())])),
                match_expressions: None,
            }),
            namespaces: Some(vec!["namespace1".to_string(), "namespace2".to_string()]),
            topology_key: "topology_key".to_string(),
        };

        let result = convert_exec_pod_anti_affinity_term(&term);
        assert_eq!(result.topology_key, "topology_key");
        assert_eq!(
            result.namespaces,
            Some(vec!["namespace1".to_string(), "namespace2".to_string()])
        );
        assert!(result.label_selector.is_some());
        assert!(result.namespace_selector.is_some());

        let label_selector = result.label_selector.unwrap();
        assert_eq!(
            label_selector.match_labels.unwrap().get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(label_selector.match_expressions.unwrap().len(), 1);

        let namespace_selector = result.namespace_selector.unwrap();
        assert_eq!(
            namespace_selector.match_labels.unwrap().get("ns_key"),
            Some(&"ns_value".to_string())
        );
    }
    #[test]

    fn test_convert_required_pod_anti_affinity_term() {
        let term = ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution {
            label_selector: Some(ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector {
                match_expressions: None,
                match_labels: Some(std::collections::BTreeMap::from([("key".to_string(), "value".to_string())])),
            }),
            namespace_selector: None,
            namespaces: Some(vec!["default".to_string()]),
            topology_key: "kubernetes.io/hostname".to_string(),
        };

        let pod_affinity_term = convert_required_pod_anti_affinity_term(&term);
        assert_eq!(pod_affinity_term.topology_key, "kubernetes.io/hostname");
        assert!(pod_affinity_term.label_selector.is_some());
    }

    #[test]
    fn test_convert_pod_anti_affinity_to_pooler_non_empty() {
        let pod_anti_affinity = create_sample_pod_anti_affinity();
        let result = convert_pod_anti_affinity_to_pooler(&pod_anti_affinity);

        assert!(result.is_some());
        let pooler_anti_affinity = result.unwrap();
        assert_eq!(
            pooler_anti_affinity
                .required_during_scheduling_ignored_during_execution
                .as_ref()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            pooler_anti_affinity
                .preferred_during_scheduling_ignored_during_execution
                .as_ref()
                .unwrap()
                .len(),
            1
        );

        let required = &pooler_anti_affinity
            .required_during_scheduling_ignored_during_execution
            .as_ref()
            .unwrap()[0];
        assert_eq!(
            required
                .label_selector
                .as_ref()
                .unwrap()
                .match_labels
                .as_ref()
                .unwrap()
                .get("key1"),
            Some(&"value1".to_string())
        );

        let preferred = &pooler_anti_affinity
            .preferred_during_scheduling_ignored_during_execution
            .as_ref()
            .unwrap()[0];
        assert_eq!(
            preferred
                .pod_affinity_term
                .label_selector
                .as_ref()
                .unwrap()
                .match_labels
                .as_ref()
                .unwrap()
                .get("key2"),
            Some(&"value2".to_string())
        );
    }

    #[test]
    fn test_convert_pod_anti_affinity_to_pooler_empty() {
        let pod_anti_affinity = PodAntiAffinity {
            required_during_scheduling_ignored_during_execution: None,
            preferred_during_scheduling_ignored_during_execution: None,
        };
        let result = convert_pod_anti_affinity_to_pooler(&pod_anti_affinity);

        assert!(result.is_some());
        let pooler_anti_affinity = result.unwrap();
        assert!(pooler_anti_affinity
            .required_during_scheduling_ignored_during_execution
            .is_none());
        assert!(pooler_anti_affinity
            .preferred_during_scheduling_ignored_during_execution
            .is_none());
    }
}
