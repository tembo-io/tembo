use crate::apis::coredb_types::CoreDB;
use crate::cloudnativepg::placement::{
    cnpg_node_affinity::{convert_node_affinity, convert_node_affinity_to_pooler},
    cnpg_pod_affinity::{convert_pod_affinity, convert_pod_affinity_to_pooler},
    cnpg_pod_anti_affinity::{convert_pod_anti_affinity, convert_pod_anti_affinity_to_pooler},
    cnpg_toleration::{convert_toleration, convert_toleration_to_pooler},
    cnpg_topology::{convert_cluster_topology_spread_constraints, convert_topo_to_pooler},
};
use crate::cloudnativepg::poolers::{
    PoolerTemplateSpecAffinity, PoolerTemplateSpecTolerations,
    PoolerTemplateSpecTopologySpreadConstraints,
};
use k8s_openapi::api::core::v1::{
    Affinity, NodeAffinity, PodAffinity, PodAntiAffinity, Toleration, TopologySpreadConstraint,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct PlacementConfig {
    pub node_selector: Option<BTreeMap<String, String>>,
    pub tolerations: Vec<Toleration>,
    pub node_affinity: Option<NodeAffinity>,
    pub pod_affinity: Option<PodAffinity>,
    pub pod_anti_affinity: Option<PodAntiAffinity>,
    pub topology_spread_constraints: Option<Vec<TopologySpreadConstraint>>,
}

// PlacementConfig is a struct that holds the affinity and topology configuration for the CNPG Cluster, Pooler
// and any Tembo specific Deployments like for AppServices.
impl PlacementConfig {
    pub fn new(core_db: &CoreDB) -> Option<Self> {
        core_db
            .spec
            .affinity_configuration
            .as_ref()
            .map(|config| PlacementConfig {
                node_selector: config.node_selector.clone(),
                tolerations: config
                    .tolerations
                    .as_ref()
                    .map_or_else(Vec::new, |tolerations| {
                        tolerations.iter().map(convert_toleration).collect()
                    }),
                node_affinity: config.node_affinity.as_ref().map(convert_node_affinity),
                pod_affinity: config
                    .additional_pod_affinity
                    .as_ref()
                    .map(convert_pod_affinity),
                pod_anti_affinity: config
                    .additional_pod_anti_affinity
                    .as_ref()
                    .map(convert_pod_anti_affinity),
                topology_spread_constraints: convert_cluster_topology_spread_constraints(
                    &core_db.spec.topology_spread_constraints,
                ),
            })
    }

    // combine_affinity_items will combine self.node_affinity, self.pod_affinity, and self.pod_anti_affinity into a single pod affinity object.
    // This is used to simplify the process of creating an affinity object for a pod or deployment.
    pub fn combine_affinity_items(&self) -> Option<Affinity> {
        let mut affinity = Affinity::default();
        if let Some(node_affinity) = &self.node_affinity {
            affinity.node_affinity = Some(node_affinity.clone());
        }
        if let Some(pod_affinity) = &self.pod_affinity {
            affinity.pod_affinity = Some(pod_affinity.clone());
        }
        if let Some(pod_anti_affinity) = &self.pod_anti_affinity {
            affinity.pod_anti_affinity = Some(pod_anti_affinity.clone());
        }
        if affinity.node_affinity.is_none()
            && affinity.pod_affinity.is_none()
            && affinity.pod_anti_affinity.is_none()
        {
            None
        } else {
            Some(affinity)
        }
    }

    // convert_pooler_tolerations Converts `Toleration` to `PoolerTemplateSpecTolerations`.
    // to be used in the PoolerTemplateSpec struct when building out a pooler.
    pub fn convert_pooler_tolerations(&self) -> Option<Vec<PoolerTemplateSpecTolerations>> {
        if self.tolerations.is_empty() {
            None
        } else {
            Some(
                self.tolerations
                    .iter()
                    .filter_map(convert_toleration_to_pooler)
                    .collect(),
            )
        }
    }

    // convert_pooler_topology_spread_constraints Converts `TopologySpreadConstraint` to `PoolerTemplateSpecTopologySpreadConstraints`.
    // to be used in the PoolerTemplateSpec struct when building out a pooler.
    pub fn convert_pooler_topology_spread_constraints(
        &self,
    ) -> Option<Vec<PoolerTemplateSpecTopologySpreadConstraints>> {
        self.topology_spread_constraints
            .as_ref()
            .and_then(|topologies| {
                if topologies.is_empty() {
                    None
                } else {
                    convert_topo_to_pooler(topologies)
                }
            })
    }

    // convert_pooler_affinity Converts `Affinity` to `PoolerTemplateSpecAffinity`.
    // to be used in the PoolerTemplateSpec struct when building out a pooler.
    pub fn convert_pooler_affinity(&self) -> Option<PoolerTemplateSpecAffinity> {
        if self.node_affinity.is_none()
            && self.pod_affinity.is_none()
            && self.pod_anti_affinity.is_none()
        {
            None
        } else {
            Some(PoolerTemplateSpecAffinity {
                node_affinity: self
                    .node_affinity
                    .as_ref()
                    .and_then(convert_node_affinity_to_pooler),
                pod_affinity: self
                    .pod_affinity
                    .as_ref()
                    .and_then(convert_pod_affinity_to_pooler),
                pod_anti_affinity: self
                    .pod_anti_affinity
                    .as_ref()
                    .and_then(convert_pod_anti_affinity_to_pooler),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{
        NodeSelector, NodeSelectorRequirement, NodeSelectorTerm, PodAffinityTerm, Toleration,
        WeightedPodAffinityTerm,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;

    fn create_node_affinity() -> NodeAffinity {
        NodeAffinity {
            required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                node_selector_terms: vec![NodeSelectorTerm {
                    match_expressions: Some(vec![NodeSelectorRequirement {
                        key: "key1".to_string(),
                        operator: "In".to_string(),
                        values: Some(vec!["value1".to_string()]),
                    }]),
                    ..Default::default()
                }],
            }),
            ..Default::default()
        }
    }

    fn create_pod_affinity() -> PodAffinity {
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
                    },
                },
            ]),
        }
    }

    fn create_pod_anti_affinity() -> PodAntiAffinity {
        PodAntiAffinity {
            required_during_scheduling_ignored_during_execution: Some(vec![PodAffinityTerm {
                label_selector: Some(LabelSelector {
                    match_labels: Some(
                        [("anti-key", "anti-value")]
                            .iter()
                            .cloned()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect(),
                    ),
                    ..Default::default()
                }),
                namespaces: Some(vec!["default-namespace".to_string()]),
                topology_key: "kubernetes.io/hostname".to_string(),
                ..Default::default()
            }]),
            preferred_during_scheduling_ignored_during_execution: Some(vec![
                WeightedPodAffinityTerm {
                    weight: 80,
                    pod_affinity_term: PodAffinityTerm {
                        label_selector: Some(LabelSelector {
                            match_labels: Some(
                                [("pref-anti-key", "pref-anti-value")]
                                    .iter()
                                    .cloned()
                                    .map(|(k, v)| (k.to_string(), v.to_string()))
                                    .collect(),
                            ),
                            ..Default::default()
                        }),
                        namespaces: Some(vec!["preferred-namespace".to_string()]),
                        topology_key: "kubernetes.io/zone".to_string(),
                        ..Default::default()
                    },
                },
            ]),
        }
    }

    fn create_tolerations() -> Vec<Toleration> {
        vec![Toleration {
            key: Some("key".to_string()),
            operator: Some("Equal".to_string()),
            value: Some("value".to_string()),
            effect: Some("NoSchedule".to_string()),
            toleration_seconds: Some(3600),
        }]
    }

    #[test]

    fn test_convert_pooler_tolerations_non_empty() {
        let tolerations = create_tolerations();

        let placement = PlacementConfig {
            tolerations,
            node_affinity: None,
            pod_affinity: None,
            pod_anti_affinity: None,
            topology_spread_constraints: None,
            node_selector: None,
        };

        let result = placement.convert_pooler_tolerations();
        assert!(
            result.is_some(),
            "Tolerations conversion should not be None"
        );
        let tolerations_result = result.unwrap();
        assert_eq!(
            tolerations_result.len(),
            1,
            "Expected exactly one toleration to be converted"
        );

        // Detailed assertions on toleration contents
        let toleration = &tolerations_result[0];
        assert_eq!(toleration.key.as_deref(), Some("key"));
        assert_eq!(toleration.operator.as_deref(), Some("Equal"));
        assert_eq!(toleration.value.as_deref(), Some("value"));
        assert_eq!(toleration.effect.as_deref(), Some("NoSchedule"));
        assert_eq!(toleration.toleration_seconds, Some(3600));
    }

    #[test]
    fn test_combine_affinity_items() {
        let node_affinity = create_node_affinity();
        let pod_affinity = create_pod_affinity();
        let pod_anti_affinity = create_pod_anti_affinity();
        let tolerations = create_tolerations();

        let placement_config = PlacementConfig {
            node_selector: None,
            tolerations,
            node_affinity: Some(node_affinity),
            pod_affinity: Some(pod_affinity),
            pod_anti_affinity: Some(pod_anti_affinity),
            topology_spread_constraints: None, // Add sample topology constraints if needed
        };

        let combined_affinity = placement_config.combine_affinity_items().unwrap();

        assert!(
            combined_affinity.node_affinity.is_some(),
            "Node affinity should be combined."
        );
        assert!(
            combined_affinity.pod_affinity.is_some(),
            "Pod affinity should be combined."
        );
        assert!(
            combined_affinity.pod_anti_affinity.is_some(),
            "Pod anti-affinity should be combined."
        );

        // Ensure that tolerations are converted correctly (this assumes a simple pass-through in your actual function)
        let converted_tolerations = placement_config.convert_pooler_tolerations().unwrap();
        assert_eq!(
            converted_tolerations.len(),
            placement_config.tolerations.len(),
            "Tolerations should be converted correctly."
        );
    }
}
