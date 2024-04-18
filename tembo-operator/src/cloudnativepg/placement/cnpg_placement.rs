use crate::apis::coredb_types::CoreDB;
use crate::cloudnativepg::placement::{
    cnpg_affinity::{convert_node_affinity, convert_pod_affinity, convert_pod_anti_affinity},
    cnpg_toleration::{convert_toleration, convert_toleration_to_pooler},
    cnpg_topology::{convert_cluster_topology_spread_constraints, convert_topo_to_pooler},
};
use crate::cloudnativepg::poolers::{
    PoolerTemplateSpecTolerations, PoolerTemplateSpecTopologySpreadConstraints,
};
use k8s_openapi::api::core::v1::{
    NodeAffinity, PodAffinity, PodAntiAffinity, Toleration, TopologySpreadConstraint,
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
}
