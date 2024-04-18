use crate::cloudnativepg::clusters::{
    ClusterAffinityAdditionalPodAffinity,
    ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
    ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
    ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
    ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecution,
    ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
    ClusterAffinityAdditionalPodAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
    ClusterAffinityAdditionalPodAntiAffinity,
    ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTerm,
    ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelector,
    ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermNamespaceSelector,
    ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecution,
    ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionLabelSelector,
    ClusterAffinityAdditionalPodAntiAffinityRequiredDuringSchedulingIgnoredDuringExecutionNamespaceSelector,
    ClusterAffinityNodeAffinity,
    ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchExpressions,
    ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchFields,
    ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchExpressions,
    ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchFields,
};
use k8s_openapi::api::core::v1::{
    NodeAffinity, NodeSelector, NodeSelectorRequirement, NodeSelectorTerm, PodAffinity,
    PodAffinityTerm, PodAntiAffinity, PreferredSchedulingTerm, WeightedPodAffinityTerm,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement};

// Start of functions needed to convert a ClusterAffinityNodeAffinity to a k8s_openapi::api::core::v1::NodeAffinity
//
// convert_node_affinity converts a ClusterAffinityNodeAffinity to a k8s_openapi::api::core::v1::NodeAffinity
// this is the meta function that calls the other conversion functions
pub fn convert_node_affinity(ca: &ClusterAffinityNodeAffinity) -> NodeAffinity {
    NodeAffinity {
        preferred_during_scheduling_ignored_during_execution: Some(convert_preferred(ca)),
        required_during_scheduling_ignored_during_execution: convert_required(ca),
    }
}

// convert_preferred converts a ClusterAffinityNodeAffinity to a Vec<PreferredSchedulingTerm>
fn convert_preferred(ca: &ClusterAffinityNodeAffinity) -> Vec<PreferredSchedulingTerm> {
    match &ca.preferred_during_scheduling_ignored_during_execution {
        Some(prefs) => prefs
            .iter()
            .map(|pref| PreferredSchedulingTerm {
                weight: pref.weight,
                preference: NodeSelectorTerm {
                    match_expressions: Some(convert_match_expressions(
                        &pref.preference.match_expressions,
                    )),
                    match_fields: Some(convert_match_fields(&pref.preference.match_fields)),
                },
            })
            .collect(),
        None => Vec::new(),
    }
}
// convert_required converts a ClusterAffinityNodeAffinity to an Option<NodeSelector>
fn convert_required(ca: &ClusterAffinityNodeAffinity) -> Option<NodeSelector> {
    ca.required_during_scheduling_ignored_during_execution
        .as_ref()
        .map(|req| NodeSelector {
            node_selector_terms: req
                .node_selector_terms
                .iter()
                .map(|term| NodeSelectorTerm {
                    match_expressions: convert_required_match_expressions(&term.match_expressions),
                    match_fields: convert_required_match_fields(&term.match_fields),
                })
                .collect(),
        })
}

// Convert match expressions safely without assuming defaults
fn convert_required_match_expressions(
    expressions: &Option<Vec<ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchExpressions>>,
) -> Option<Vec<NodeSelectorRequirement>> {
    expressions.as_ref().map(|exprs| {
        exprs
            .iter()
            .map(|expr| {
                NodeSelectorRequirement {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: expr.values.as_ref().cloned(), // Use cloned to safely handle the Option
                }
            })
            .collect()
    })
}

// Convert match fields safely without assuming defaults
fn convert_required_match_fields(
    fields: &Option<Vec<ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchFields>>,
) -> Option<Vec<NodeSelectorRequirement>> {
    fields.as_ref().map(|flds| {
        flds.iter()
            .map(|field| {
                NodeSelectorRequirement {
                    key: field.key.clone(),
                    operator: field.operator.clone(),
                    values: field.values.as_ref().cloned(), // Use cloned to safely handle the Option
                }
            })
            .collect()
    })
}

// convert_match_expressions converts a ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchExpressions to a Vec<NodeSelectorRequirement>
fn convert_match_expressions(
    expressions: &Option<Vec<ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchExpressions>>,
) -> Vec<NodeSelectorRequirement> {
    expressions.as_ref().map_or(Vec::new(), |exprs| {
        exprs
            .iter()
            .map(|expr| NodeSelectorRequirement {
                key: expr.key.clone(),
                operator: expr.operator.clone(),
                values: Some(
                    expr.values
                        .as_ref()
                        .map_or_else(Vec::new, |vals| vals.clone()),
                ),
            })
            .collect()
    })
}

// convert_match_fields converts a ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchFields to a Vec<NodeSelectorRequirement>
fn convert_match_fields(
    fields: &Option<Vec<ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchFields>>,
) -> Vec<NodeSelectorRequirement> {
    fields.as_ref().map_or(Vec::new(), |flds| {
        flds.iter()
            .map(|field| NodeSelectorRequirement {
                key: field.key.clone(),
                operator: field.operator.clone(),
                values: Some(
                    field
                        .values
                        .as_ref()
                        .map_or_else(Vec::new, |vals| vals.clone()),
                ),
            })
            .collect()
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloudnativepg::clusters::{
        ClusterAffinityAdditionalPodAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions,
        ClusterAffinityAdditionalPodAntiAffinityPreferredDuringSchedulingIgnoredDuringExecutionPodAffinityTermLabelSelectorMatchExpressions,
        ClusterAffinityTolerations,
    };
    use crate::cloudnativepg::placement::cnpg_toleration::convert_toleration;

    #[test]
    fn test_convert_toleration() {
        let tol = ClusterAffinityTolerations {
            effect: Some("NoSchedule".to_string()),
            key: Some("key1".to_string()),
            operator: Some("Exists".to_string()),
            toleration_seconds: Some(3600),
            value: Some("value1".to_string()),
        };

        let result = convert_toleration(&tol);

        assert_eq!(result.effect, Some("NoSchedule".to_string()));
        assert_eq!(result.key, Some("key1".to_string()));
        assert_eq!(result.operator, Some("Exists".to_string()));
        assert_eq!(result.toleration_seconds, Some(3600));
        assert_eq!(result.value, Some("value1".to_string()));
    }

    #[test]
    fn test_convert_node_affinity_empty() {
        let ca = ClusterAffinityNodeAffinity {
            preferred_during_scheduling_ignored_during_execution: None,
            required_during_scheduling_ignored_during_execution: None,
        };

        let result = convert_node_affinity(&ca);
        // assert!(result
        //     .preferred_during_scheduling_ignored_during_execution
        //     .expect("preferred_during_scheduling_ignored_during_execution should be Some"));
        assert!(result
            .required_during_scheduling_ignored_during_execution
            .is_none());
    }

    #[test]
    fn test_convert_required_match_expressions_empty() {
        let expressions = None;
        let result = convert_required_match_expressions(&expressions);
        assert_eq!(result, None);
    }

    #[test]
    fn test_convert_required_match_fields_empty() {
        let fields = None;
        let result = convert_required_match_fields(&fields);
        assert_eq!(result, None);
    }

    #[test]
    fn test_convert_required_match_expressions() {
        let expressions = Some(vec![
            ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchExpressions {
                key: "key1".to_string(),
                operator: "In".to_string(),
                values: Some(vec!["value1".to_string(), "value2".to_string()]),
            },
        ]);

        let result = convert_required_match_expressions(&expressions).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "key1");
        assert_eq!(result[0].operator, "In");
        assert_eq!(
            result[0].values,
            Some(vec!["value1".to_string(), "value2".to_string()])
        );
    }

    #[test]
    fn test_convert_required_match_fields() {
        let fields = Some(vec![
            ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchFields {
                key: "field1".to_string(),
                operator: "Exists".to_string(),
                values: None,
            },
        ]);

        let result = convert_required_match_fields(&fields).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "field1");
        assert_eq!(result[0].operator, "Exists");
        assert!(result[0].values.is_none());
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
}
