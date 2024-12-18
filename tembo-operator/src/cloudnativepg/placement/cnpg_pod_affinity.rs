use crate::cloudnativepg::clusters::{
    ClusterAffinityAdditionalPodAffinity,
    ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
    ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
    ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
    ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecution,
    ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
    ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
};
use crate::cloudnativepg::poolers::{
    PoolerTemplateSpecAffinityPodAffinity,
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecution,
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions,
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelectorMatchExpressions,
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecution,
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelectorMatchExpressions,
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelectorMatchExpressions,
};
use k8s_openapi::api::core::v1::{PodAffinity, PodAffinityTerm, WeightedPodAffinityTerm};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement};

// Start of helper functions to convert a ClusterAffinityPodAffinity to a k8s_openapi::api::core::v1::PodAffinity
//
// convert_pod_affinity_term converts a ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm to a k8s_openapi::api::core::v1::PodAffinityTerm
fn convert_exec_pod_affinity_term(
    term: &ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
) -> PodAffinityTerm {
    PodAffinityTerm {
        label_selector: term
            .label_selector
            .as_ref()
            .map(convert_affinity_term_label_selector),
        namespace_selector: term
            .namespace_selector
            .as_ref()
            .map(convert_affinity_term_namespace_selector),
        namespaces: term.namespaces.clone(),
        topology_key: term.topology_key.clone(),
        ..Default::default()
    }
}

// convert_required_pod_affinity_term converts a ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecution to a PodAffinityTerm
fn convert_required_pod_affinity_term(
    term: &ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecution,
) -> PodAffinityTerm {
    PodAffinityTerm {
        label_selector: term
            .label_selector
            .as_ref()
            .map(convert_affinity_exec_label_selector),
        namespace_selector: term
            .namespace_selector
            .as_ref()
            .map(convert_affinity_exec_namespace_selector),
        namespaces: term.namespaces.clone(),
        topology_key: term.topology_key.clone(),
        ..Default::default()
    }
}

// convert_exec_label_selector converts a ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector to a LabelSelector
fn convert_affinity_exec_label_selector(
    selector: &ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
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

// convert_term_label_selector converts a ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector to a LabelSelector
fn convert_affinity_term_label_selector(
    selector: &ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
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

// convert_exec_namespace_selector converts a ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector to a LabelSelector
fn convert_affinity_exec_namespace_selector(
    ns_selector: &ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
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

// convert_term_namespace_selector converts a ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector to a LabelSelector
fn convert_affinity_term_namespace_selector(
    ns_selector: &ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
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

// convert_pod_affinity converts a ClusterAffinityAdditionalPodAffinity to a PodAffinity
pub fn convert_pod_affinity(
    cluster_affinity: &ClusterAffinityAdditionalPodAffinity,
) -> PodAffinity {
    PodAffinity {
        required_during_scheduling_ignored_during_execution: cluster_affinity
            .required_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|req| req.iter().map(convert_required_pod_affinity_term).collect()),
        preferred_during_scheduling_ignored_during_execution: cluster_affinity
            .preferred_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|prefs| {
                prefs
                    .iter()
                    .map(|pref| WeightedPodAffinityTerm {
                        weight: pref.weight,
                        pod_affinity_term: convert_exec_pod_affinity_term(&pref.pod_affinity_term),
                    })
                    .collect()
            }),
    }
}

// Start functions that do conversions to the PoolerTemplateSpec structs
//
// convert_pod_affinity_to_pooler converts a PodAffinity to a PoolerTemplateSpecAffinityPodAffinity struct
// to be used in the PoolerTemplateSpec struct when building out a pooler.
pub fn convert_pod_affinity_to_pooler(
    pod_affinity: &PodAffinity,
) -> Option<PoolerTemplateSpecAffinityPodAffinity> {
    Some(PoolerTemplateSpecAffinityPodAffinity {
        required_during_scheduling_ignored_during_execution: pod_affinity
            .required_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|req| {
                req.iter()
                    .map(convert_required_pod_affinity_term_to_pooler)
                    .collect()
            }),
        preferred_during_scheduling_ignored_during_execution: pod_affinity
            .preferred_during_scheduling_ignored_during_execution
            .as_ref()
            .map(|prefs| {
                prefs
                    .iter()
                    .map(|pref| PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecution {
                        pod_affinity_term: convert_preferred_pod_affinity_term_to_pooler(&pref.pod_affinity_term),
                        weight: pref.weight,
                    })
                    .collect()
            }),
    })
}

// convert_required_pod_affinity_term_to_pooler converts a PodAffinityTerm to a PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecution
// to be used in the PoolerTemplateSpecAffinityPodAffinity struct when building out a pooler.
fn convert_required_pod_affinity_term_to_pooler(
    term: &PodAffinityTerm,
) -> PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecution {
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecution {
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

// convert_preferred_pod_affinity_term_to_pooler converts a PodAffinityTerm to a PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecution
// to be used in the PoolerTemplateSpecAffinityPodAffinity struct when building out a pooler.
fn convert_preferred_pod_affinity_term_to_pooler(
    term: &PodAffinityTerm,
) -> PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm{
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm {
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

// convert_preferred_namespace_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector
// to be used in the PoolerTemplateSpecAffinityPodAffinity struct when building out a pooler.
fn convert_preferred_namespace_selector_to_pooler(
    ns_selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector{
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector {
        match_labels: ns_selector.match_labels.clone(),
        match_expressions: ns_selector.match_expressions.as_ref().map(|exprs| {
            exprs.iter().map(|expr| {
                PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelectorMatchExpressions {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: expr.values.clone(),
                }
            }).collect()
        }),
    }
}

// convert_preferred_label_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector
// to be used in the PoolerTemplateSpecAffinityPodAffinity struct when building out a pooler.
fn convert_preferred_label_selector_to_pooler(
    selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector{
    PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|exprs| exprs.iter().map(|expr| {
            PoolerTemplateSpecAffinityPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions {
                key: expr.key.clone(),
                operator: expr.operator.clone(),
                values: expr.values.clone(),
            }
        }).collect()),
    }
}

// convert_required_namespace_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector
// to be used in the PoolerTemplateSpecAffinityPodAffinity struct when building out a pooler.
fn convert_required_namespace_selector_to_pooler(
    ns_selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector{
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector {
        match_labels: ns_selector.match_labels.clone(),
        match_expressions: ns_selector.match_expressions.as_ref().map(|exprs| {
            exprs.iter().map(|expr| {
                PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelectorMatchExpressions {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: expr.values.clone(),
                }
            }).collect()
        }),
    }
}

// convert_required_label_selector_to_pooler converts a LabelSelector to a PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector
// to be used in the PoolerTemplateSpecAffinityPodAffinity struct when building out a pooler.
fn convert_required_label_selector_to_pooler(
    selector: &LabelSelector,
) -> PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector
{
    PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|exprs| {
            exprs.iter().map(|expr| {
                PoolerTemplateSpecAffinityPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelectorMatchExpressions {
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
    use crate::cloudnativepg::clusters::ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions;

    fn create_sample_pod_affinity() -> PodAffinity {
        PodAffinity {
            required_during_scheduling_ignored_during_execution: Some(vec![PodAffinityTerm {
                label_selector: Some(LabelSelector {
                    match_labels: Some(
                        [("key", "value")]
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
    fn test_convert_pod_affinity_term_full() {
        let term = ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm {
            label_selector: Some(ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector {
                match_labels: Some(std::collections::BTreeMap::from([
                    ("key1".to_string(), "value1".to_string()),
                    ("key2".to_string(), "value2".to_string()),
                ])),
                match_expressions: Some(vec![
                    ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions {
                        key: "exp_key".to_string(),
                        operator: "In".to_string(),
                        values: Some(vec!["val1".to_string(), "val2".to_string()]),
                    },
                ]),
            }),
            namespace_selector: Some(ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector {
                match_labels: Some(std::collections::BTreeMap::from([("ns_key".to_string(), "ns_value".to_string())])),
                match_expressions: None,
            }),
            namespaces: Some(vec!["namespace1".to_string(), "namespace2".to_string()]),
            topology_key: "topology_key".to_string(),
        };

        let result = convert_exec_pod_affinity_term(&term);
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

    // Test converting an empty PodAffinityTerm
    #[test]
    fn test_convert_pod_affinity_term_empty() {
        let term = ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm {
            label_selector: None,
            namespace_selector: None,
            namespaces: None,
            topology_key: "".to_string(),
        };

        let result = convert_exec_pod_affinity_term(&term);
        assert_eq!(result.topology_key, "");
        assert!(result.label_selector.is_none());
        assert!(result.namespace_selector.is_none());
        assert!(result.namespaces.is_none());
    }

    #[test]
    fn test_convert_pod_affinity_to_pooler_non_empty() {
        let pod_affinity = create_sample_pod_affinity();
        let result = convert_pod_affinity_to_pooler(&pod_affinity);

        assert!(result.is_some());
        let pooler_pod_affinity = result.unwrap();
        assert_eq!(
            pooler_pod_affinity
                .required_during_scheduling_ignored_during_execution
                .as_ref()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            pooler_pod_affinity
                .preferred_during_scheduling_ignored_during_execution
                .as_ref()
                .unwrap()
                .len(),
            1
        );

        let required = &pooler_pod_affinity
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
                .get("key"),
            Some(&"value".to_string())
        );

        let preferred = &pooler_pod_affinity
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
    fn test_convert_pod_affinity_to_pooler_empty() {
        let pod_affinity = PodAffinity {
            required_during_scheduling_ignored_during_execution: None,
            preferred_during_scheduling_ignored_during_execution: None,
        };
        let result = convert_pod_affinity_to_pooler(&pod_affinity);

        assert!(result.is_some());
        let pooler_pod_affinity = result.unwrap();
        assert!(pooler_pod_affinity
            .required_during_scheduling_ignored_during_execution
            .is_none());
        assert!(pooler_pod_affinity
            .preferred_during_scheduling_ignored_during_execution
            .is_none());
    }
}
