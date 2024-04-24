use crate::cloudnativepg::clusters::{
    ClusterAffinityNodeAffinity,
    ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchExpressions,
    ClusterAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchFields,
    ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchExpressions,
    ClusterAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchFields,
};
use crate::cloudnativepg::poolers::{
    PoolerTemplateSpecAffinityNodeAffinity,
    PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecution,
    PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreference,
    PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchExpressions,
    PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchFields,
    PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecution,
    PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTerms,
    PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchExpressions,
    PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchFields,
};
use k8s_openapi::api::core::v1::{
    NodeAffinity, NodeSelector, NodeSelectorRequirement, NodeSelectorTerm, PreferredSchedulingTerm,
};

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

// convert_node_affinity_to_pooler converts a NodeAffinity to a PoolerTemplateSpecAffinityNodeAffinity
// to be used in the PoolerTemplateSpec struct when creating a new pooler.
pub fn convert_node_affinity_to_pooler(
    node_affinity: &NodeAffinity,
) -> Option<PoolerTemplateSpecAffinityNodeAffinity> {
    if node_affinity
        .preferred_during_scheduling_ignored_during_execution
        .is_none()
        && node_affinity
            .required_during_scheduling_ignored_during_execution
            .is_none()
    {
        None
    } else {
        Some(PoolerTemplateSpecAffinityNodeAffinity {
            required_during_scheduling_ignored_during_execution: node_affinity.required_during_scheduling_ignored_during_execution.as_ref().map(|req| {
                PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecution {
                    node_selector_terms: convert_node_selector_terms_to_pooler(&req.node_selector_terms),
                }
            }),
            preferred_during_scheduling_ignored_during_execution: node_affinity.preferred_during_scheduling_ignored_during_execution.as_ref().map(|prefs| {
                convert_preferred_scheduling_terms_to_pooler(prefs)
            }),
        })
    }
}

// convert_node_selector_terms_to_pooler converts a Vec<NodeSelectorTerm> to a Vec<PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTerms>
// to be used in the PoolerTemplateSpecAffinityNodeAffinity struct when creating a new pooler.
fn convert_node_selector_terms_to_pooler(
    terms: &[NodeSelectorTerm],
) -> Vec<PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTerms>{
    terms.iter().map(|term| {
        PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTerms {
            match_expressions: term.match_expressions.as_ref().map(|expressions| {
                expressions.iter().map(|expr| {
                    PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchExpressions {
                        key: expr.key.clone(),
                        operator: expr.operator.clone(),
                        values: expr.values.clone(),
                    }
                }).collect()
            }),
            match_fields: term.match_fields.as_ref().map(|fields| {
                fields.iter().map(|field| {
                    PoolerTemplateSpecAffinityNodeAffinityRequiredDuringSchedulingIgnoredDuringExecutionNodeSelectorTermsMatchFields {
                        key: field.key.clone(),
                        operator: field.operator.clone(),
                        values: field.values.clone(),
                    }
                }).collect()
            }),
        }
    }).collect()
}

// convert_preferred_scheduling_terms_to_pooler converts a Vec<PreferredSchedulingTerm> to a Vec<PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecution>
// to be used in the PoolerTemplateSpecAffinityNodeAffinity struct when creating a new pooler.
fn convert_preferred_scheduling_terms_to_pooler(
    terms: &[PreferredSchedulingTerm],
) -> Vec<PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecution> {
    terms.iter().map(|term| {
        PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecution {
            preference: PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreference {
                match_expressions: term.preference.match_expressions.as_ref().map(|expressions| {
                    expressions.iter().map(|expr| {
                        PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchExpressions {
                            key: expr.key.clone(),
                            operator: expr.operator.clone(),
                            values: expr.values.clone(),
                        }
                    }).collect()
                }),
                match_fields: term.preference.match_fields.as_ref().map(|fields| {
                    fields.iter().map(|field| {
                        PoolerTemplateSpecAffinityNodeAffinityPreferredDuringSchedulingIgnoredDuringExecutionPreferenceMatchFields {
                            key: field.key.clone(),
                            operator: field.operator.clone(),
                            values: field.values.clone(),
                        }
                    }).collect()
                }),
            },
            weight: term.weight,
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_sample_node_affinity() -> NodeAffinity {
        NodeAffinity {
            required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                node_selector_terms: vec![NodeSelectorTerm {
                    match_expressions: Some(vec![NodeSelectorRequirement {
                        key: "region".to_string(),
                        operator: "In".to_string(),
                        values: Some(vec!["us-west-1".to_string()]),
                    }]),
                    ..Default::default()
                }],
            }),
            preferred_during_scheduling_ignored_during_execution: Some(vec![
                PreferredSchedulingTerm {
                    weight: 100,
                    preference: NodeSelectorTerm {
                        match_expressions: Some(vec![NodeSelectorRequirement {
                            key: "zone".to_string(),
                            operator: "In".to_string(),
                            values: Some(vec!["us-west-1a".to_string()]),
                        }]),
                        ..Default::default()
                    },
                },
            ]),
        }
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
    fn test_convert_node_affinity_to_pooler_non_empty() {
        let node_affinity = create_sample_node_affinity();
        let result = convert_node_affinity_to_pooler(&node_affinity);

        assert!(result.is_some());
        let pooler_node_affinity = result.unwrap();
        assert!(pooler_node_affinity
            .required_during_scheduling_ignored_during_execution
            .is_some());
        assert!(pooler_node_affinity
            .preferred_during_scheduling_ignored_during_execution
            .is_some());

        let required = &pooler_node_affinity
            .required_during_scheduling_ignored_during_execution
            .unwrap();
        assert_eq!(required.node_selector_terms.len(), 1);
        assert_eq!(
            required.node_selector_terms[0]
                .match_expressions
                .as_ref()
                .unwrap()[0]
                .key,
            "region"
        );

        let preferred = &pooler_node_affinity
            .preferred_during_scheduling_ignored_during_execution
            .unwrap()[0];
        assert_eq!(preferred.weight, 100);
        assert_eq!(
            preferred.preference.match_expressions.as_ref().unwrap()[0].key,
            "zone"
        );
    }

    #[test]
    fn test_convert_node_affinity_to_pooler_empty() {
        let node_affinity = NodeAffinity {
            required_during_scheduling_ignored_during_execution: None,
            preferred_during_scheduling_ignored_during_execution: None,
        };
        let result = convert_node_affinity_to_pooler(&node_affinity);

        assert!(result.is_none());
    }
}
