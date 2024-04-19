use crate::cloudnativepg::clusters::{
    ClusterTopologySpreadConstraints, ClusterTopologySpreadConstraintsLabelSelector,
};
use crate::cloudnativepg::poolers::{
    PoolerTemplateSpecTopologySpreadConstraints,
    PoolerTemplateSpecTopologySpreadConstraintsLabelSelector,
    PoolerTemplateSpecTopologySpreadConstraintsLabelSelectorMatchExpressions,
};
use k8s_openapi::api::core::v1::TopologySpreadConstraint;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement};

pub fn convert_cluster_topology_spread_constraints(
    cluster_constraints: &Option<Vec<ClusterTopologySpreadConstraints>>,
) -> Option<Vec<TopologySpreadConstraint>> {
    cluster_constraints.as_ref().map(|constraints| {
        constraints
            .iter()
            .map(|constraint| TopologySpreadConstraint {
                label_selector: constraint
                    .label_selector
                    .as_ref()
                    .map(convert_topo_label_selector),
                max_skew: constraint.max_skew,
                min_domains: constraint.min_domains,
                node_affinity_policy: constraint.node_affinity_policy.clone(),
                node_taints_policy: constraint.node_taints_policy.clone(),
                topology_key: constraint.topology_key.clone(),
                when_unsatisfiable: constraint.when_unsatisfiable.clone(),
                match_label_keys: constraint.match_label_keys.clone(),
            })
            .collect()
    })
}

fn convert_topo_label_selector(
    selector: &ClusterTopologySpreadConstraintsLabelSelector,
) -> LabelSelector {
    LabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|expressions| {
            expressions
                .iter()
                .map(|expr| LabelSelectorRequirement {
                    key: expr.key.clone(),
                    operator: expr.operator.clone(),
                    values: expr.values.clone(),
                })
                .collect()
        }),
    }
}

pub fn convert_topo_to_pooler(
    topologies: &[TopologySpreadConstraint],
) -> Option<Vec<PoolerTemplateSpecTopologySpreadConstraints>> {
    if topologies.is_empty() {
        None
    } else {
        Some(
            topologies
                .iter()
                .map(|topo| PoolerTemplateSpecTopologySpreadConstraints {
                    label_selector: topo
                        .label_selector
                        .as_ref()
                        .map(convert_topo_label_selector_to_pooler),
                    max_skew: topo.max_skew,
                    min_domains: topo.min_domains,
                    node_affinity_policy: topo.node_affinity_policy.clone(),
                    node_taints_policy: topo.node_taints_policy.clone(),
                    topology_key: topo.topology_key.clone(),
                    when_unsatisfiable: topo.when_unsatisfiable.clone(),
                    match_label_keys: topo.match_label_keys.clone(),
                })
                .collect(),
        )
    }
}

// Function to convert k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector
// to PoolerTemplateSpecTopologySpreadConstraintsLabelSelector
fn convert_topo_label_selector_to_pooler(
    selector: &LabelSelector,
) -> PoolerTemplateSpecTopologySpreadConstraintsLabelSelector {
    PoolerTemplateSpecTopologySpreadConstraintsLabelSelector {
        match_labels: selector.match_labels.clone(),
        match_expressions: selector.match_expressions.as_ref().map(|expressions| {
            expressions
                .iter()
                .map(|expr| {
                    PoolerTemplateSpecTopologySpreadConstraintsLabelSelectorMatchExpressions {
                        key: expr.key.clone(),
                        operator: expr.operator.clone(),
                        values: expr.values.clone(),
                    }
                })
                .collect()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloudnativepg::clusters::ClusterTopologySpreadConstraintsLabelSelectorMatchExpressions;
    use std::collections::BTreeMap;

    #[test]
    fn test_convert_full() {
        let cluster_constraints = Some(vec![ClusterTopologySpreadConstraints {
            label_selector: Some(ClusterTopologySpreadConstraintsLabelSelector {
                match_labels: Some(BTreeMap::from([("key1".to_string(), "value1".to_string())])),
                match_expressions: Some(vec![
                    ClusterTopologySpreadConstraintsLabelSelectorMatchExpressions {
                        key: "exp_key".to_string(),
                        operator: "In".to_string(),
                        values: Some(vec!["val1".to_string(), "val2".to_string()]),
                    },
                ]),
            }),
            match_label_keys: Some(vec!["label_key1".to_string()]),
            max_skew: 1,
            min_domains: Some(2),
            node_affinity_policy: Some("Honor".to_string()),
            node_taints_policy: Some("Ignore".to_string()),
            topology_key: "topology_key".to_string(),
            when_unsatisfiable: "DoNotSchedule".to_string(),
        }]);

        let result = convert_cluster_topology_spread_constraints(&cluster_constraints).unwrap();
        assert_eq!(result.len(), 1);
        let constraint = &result[0];
        assert_eq!(constraint.max_skew, 1);
        assert_eq!(constraint.topology_key, "topology_key");
        assert_eq!(constraint.when_unsatisfiable, "DoNotSchedule");
        assert_eq!(constraint.min_domains, Some(2));
        assert_eq!(constraint.node_affinity_policy, Some("Honor".to_string()));
        assert_eq!(constraint.node_taints_policy, Some("Ignore".to_string()));

        let label_selector = constraint.label_selector.as_ref().unwrap();
        assert_eq!(
            label_selector.match_labels.as_ref().unwrap().get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(label_selector.match_expressions.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_convert_topology_spread_constraints_none() {
        let constraints = None;
        let result = convert_cluster_topology_spread_constraints(&constraints);
        assert!(
            result.is_none(),
            "Expected no topology constraints to be converted."
        );
    }

    #[test]
    fn test_convert_topology_spread_constraints_empty_vector() {
        let constraints = Some(Vec::new());
        let result = convert_cluster_topology_spread_constraints(&constraints);
        assert_eq!(
            result,
            Some(Vec::new()),
            "Expected an empty vector of topology constraints."
        );
    }
}
