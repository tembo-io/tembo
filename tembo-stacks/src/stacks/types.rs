use crate::stacks::config_engines::{
    mq_config_engine, olap_config_engine, standard_config_engine, ConfigEngine,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tembo_controller::{
    apis::postgres_parameters::PgConfig,
    app_service::types::AppService,
    defaults::ImagePerPgVersion,
    defaults::{default_images, default_repository},
    extensions::types::{Extension, TrunkInstall},
    postgres_exporter::QueryConfig,
};
use utoipa::ToSchema;

#[derive(
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    PartialEq,
    ToSchema,
    strum_macros::EnumIter,
    strum_macros::Display,
)]
pub enum StackType {
    API,
    DataWarehouse,
    Geospatial,
    MachineLearning,
    MessageQueue,
    MongoAlternative,
    OLAP,
    #[default]
    OLTP,
    RAG,
    Standard,
    Timeseries,
    VectorDB,
}

impl std::str::FromStr for StackType {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "API" => Ok(StackType::API),
            "DataWarehouse" => Ok(StackType::DataWarehouse),
            "Geospatial" => Ok(StackType::Geospatial),
            "MachineLearning" => Ok(StackType::MachineLearning),
            "MessageQueue" => Ok(StackType::MessageQueue),
            "MongoAlternative" => Ok(StackType::MongoAlternative),
            "OLAP" => Ok(StackType::OLAP),
            "OLTP" => Ok(StackType::OLTP),
            "RAG" => Ok(StackType::RAG),
            "Standard" => Ok(StackType::Standard),
            "Timeseries" => Ok(StackType::Timeseries),
            "VectorDB" => Ok(StackType::VectorDB),
            _ => Err("invalid value"),
        }
    }
}

impl StackType {
    pub fn as_str(&self) -> &str {
        match self {
            StackType::API => "API",
            StackType::DataWarehouse => "DataWarehouse",
            StackType::Geospatial => "Geospatial",
            StackType::MachineLearning => "MachineLearning",
            StackType::MessageQueue => "MessageQueue",
            StackType::MongoAlternative => "MongoAlternative",
            StackType::OLAP => "OLAP",
            StackType::OLTP => "OLTP",
            StackType::RAG => "RAG",
            StackType::Standard => "Standard",
            StackType::Timeseries => "Timeseries",
            StackType::VectorDB => "VectorDB",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, ToSchema)]
pub struct Stack {
    pub name: String,
    /// specifies any resource constraints that should be applied to an instance of the Stack
    pub compute_constraints: Option<ComputeConstraint>,
    pub description: Option<String>,
    /// Organization hosting the Docker images used in this stack
    /// Default: "tembo"
    #[serde(default = "default_organization")]
    pub organization: String,
    #[serde(default = "default_stack_repository")]
    pub repository: String,
    /// The Docker images to use for each supported Postgres versions
    ///
    /// Default:
    ///     14: "standard-cnpg:14-a0a5ab5"
    ///     15: "standard-cnpg:15-a0a5ab5"
    ///     16: "standard-cnpg:16-a0a5ab5"
    #[serde(default = "default_images")]
    pub images: ImagePerPgVersion,
    pub stack_version: Option<String>,
    pub trunk_installs: Option<Vec<TrunkInstall>>,
    pub extensions: Option<Vec<Extension>>,
    /// Postgres metric definition specific to the Stack
    pub postgres_metrics: Option<QueryConfig>,
    /// configs are strongly typed so that they can be programmatically transformed
    pub postgres_config: Option<Vec<PgConfig>>,
    #[serde(default = "default_config_engine")]
    pub postgres_config_engine: Option<ConfigEngine>,
    /// external application services
    pub infrastructure: Option<Infrastructure>,
    #[serde(rename = "appServices")]
    pub app_services: Option<Vec<AppService>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct ComputeConstraint {
    pub min: Option<ComputeResource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct ComputeResource {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

impl Stack {
    // https://www.postgresql.org/docs/current/runtime-config-resource.html#RUNTIME-CONFIG-RESOURCE-MEMORY
    pub fn runtime_config(&self) -> Option<Vec<PgConfig>> {
        match &self.postgres_config_engine {
            Some(ConfigEngine::Standard) => Some(standard_config_engine(self)),
            Some(ConfigEngine::OLAP) => Some(olap_config_engine(self)),
            Some(ConfigEngine::MQ) => Some(mq_config_engine(self)),
            None => Some(standard_config_engine(self)),
        }
    }
}

fn default_organization() -> String {
    "tembo".into()
}

fn default_stack_repository() -> String {
    default_repository()
}

fn default_config_engine() -> Option<ConfigEngine> {
    Some(ConfigEngine::Standard)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, ToSchema)]
pub struct Infrastructure {
    // generic specs
    #[serde(default = "default_cpu")]
    pub cpu: String,
    #[serde(default = "default_memory")]
    pub memory: String,
    #[serde(default = "default_storage")]
    pub storage: String,
}

fn default_cpu() -> String {
    "1".to_owned()
}

fn default_memory() -> String {
    "1Gi".to_owned()
}

fn default_storage() -> String {
    "10Gi".to_owned()
}

#[cfg(test)]
mod tests {
    use crate::stacks::{
        get_stack,
        types::{Infrastructure, StackType},
    };
    use strum::IntoEnumIterator;
    use tembo_controller::apis::postgres_parameters::PgConfig;

    #[test]
    fn test_stacks_definitions() {
        let mut mq = get_stack(StackType::MessageQueue);
        let infra = Infrastructure {
            cpu: "1".to_string(),
            memory: "1Gi".to_string(),
            storage: "10Gi".to_string(),
        };
        mq.infrastructure = Some(infra);

        // testing the default instance configurations
        let runtime_configs = mq.runtime_config().expect("expected configs");
        // convert to vec to hashmap because order is not guaranteed
        let hm: std::collections::HashMap<String, PgConfig> = runtime_configs
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        let shared_buffers = hm.get("shared_buffers").unwrap();
        assert_eq!(shared_buffers.name, "shared_buffers");
        assert_eq!(shared_buffers.value.to_string(), "614MB");
        let max_connections = hm.get("max_connections").unwrap();
        assert_eq!(max_connections.name, "max_connections");
        assert_eq!(max_connections.value.to_string(), "107");
        assert!(mq.postgres_metrics.is_some());
        assert!(mq.postgres_config.is_some());
        let mq_metrics = mq.postgres_metrics.unwrap();
        assert_eq!(mq_metrics.queries.len(), 1);
        assert!(mq_metrics.queries.contains_key("pgmq"));
        assert!(mq_metrics.queries["pgmq"].master);
        assert_eq!(mq_metrics.queries["pgmq"].metrics.len(), 5);

        let mut std = get_stack(StackType::Standard);
        let infra = Infrastructure {
            cpu: "1".to_string(),
            memory: "2Gi".to_string(),
            storage: "10Gi".to_string(),
        };
        std.infrastructure = Some(infra);
        println!("STD: {:#?}", std);

        let runtime_configs = std.runtime_config().expect("expected configs");
        let hm: std::collections::HashMap<String, PgConfig> = runtime_configs
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        let shared_buffers = hm.get("shared_buffers").unwrap();
        assert_eq!(shared_buffers.name, "shared_buffers");
        assert_eq!(shared_buffers.value.to_string(), "512MB");
    }

    #[test]
    fn test_all_stack_deserialization() {
        for stack in StackType::iter() {
            match stack {
                StackType::API => {
                    get_stack(StackType::API);
                }
                StackType::DataWarehouse => {
                    get_stack(StackType::DataWarehouse);
                }
                StackType::Geospatial => {
                    get_stack(StackType::Geospatial);
                }
                StackType::MachineLearning => {
                    get_stack(StackType::MachineLearning);
                }
                StackType::MessageQueue => {
                    get_stack(StackType::MessageQueue);
                }
                StackType::MongoAlternative => {
                    get_stack(StackType::MongoAlternative);
                }
                StackType::OLAP => {
                    get_stack(StackType::OLAP);
                }
                StackType::OLTP => {
                    get_stack(StackType::OLTP);
                }
                StackType::RAG => {
                    get_stack(StackType::RAG);
                }
                StackType::Standard => {
                    get_stack(StackType::Standard);
                }
                StackType::Timeseries => {
                    get_stack(StackType::Timeseries);
                }
                StackType::VectorDB => {
                    get_stack(StackType::VectorDB);
                }
            }
        }
    }

    #[test]
    fn test_all_stack_variants() {
        for variant in StackType::iter() {
            let stack_str = variant.as_str();
            // from string back to StackType
            let stack_type = stack_str.parse::<StackType>();
            assert!(
                stack_type.is_ok(),
                "stack type missing from_str {:?}",
                stack_str
            )
        }
    }

    #[test]
    fn test_compute_constraints() {
        for variant in StackType::iter() {
            let stack = get_stack(variant.clone());
            let maybe_constraints = stack.compute_constraints;
            if variant == StackType::MachineLearning {
                // ML stack is only stack currently with constraints
                let constraints = maybe_constraints.expect("missing ML constraints");
                let min_constraint = constraints.min.expect("missing min constraint");
                assert_eq!(min_constraint.cpu, Some("2".to_string()));
                assert_eq!(min_constraint.memory, Some("4Gi".to_string()));
            } else {
                // only ML has compute constraints
                assert!(maybe_constraints.is_none());
            }
        }
    }
}
