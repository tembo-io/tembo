use crate::{
    apis::postgres_parameters::PgConfig,
    app_service::types::AppService,
    defaults::{default_images, default_repository},
    extensions::types::{Extension, TrunkInstall},
    postgres_exporter::QueryConfig,
    stacks::config_engines::{
        mq_config_engine, olap_config_engine, standard_config_engine, ConfigEngine,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, ToSchema)]
pub struct Stack {
    pub name: String,
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

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct ComputeConstraint {
    pub min: Option<ComputeResource>,
    pub max: Option<ComputeResource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct ComputeResource {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, ToSchema)]
pub struct ImagePerPgVersion {
    #[serde(rename = "14")]
    pub pg14: String,
    #[serde(rename = "15")]
    pub pg15: String,
    #[serde(rename = "16")]
    pub pg16: String,
}
