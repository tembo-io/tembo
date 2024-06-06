use k8s_openapi::api::core::v1::ResourceRequirements;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tembo_controller::{
    apis::postgres_parameters::PgConfig,
    app_service::types::{AppService, EnvVar},
    extensions::types::{Extension, TrunkInstall},
};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct App {
    pub name: AppType,
    #[serde(rename = "appServices")]
    pub app_services: Option<Vec<AppService>>,
    pub trunk_installs: Option<Vec<TrunkInstall>>,
    pub extensions: Option<Vec<Extension>>,
    pub postgres_config: Option<Vec<PgConfig>>,
}

pub struct MergedConfigs {
    pub extensions: Option<Vec<Extension>>,
    pub trunk_installs: Option<Vec<TrunkInstall>>,
    pub app_services: Option<Vec<AppService>>,
    pub pg_configs: Option<Vec<PgConfig>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub enum AppType {
    #[serde(rename = "ai-proxy")]
    AIProxy(Option<AppConfig>),
    #[serde(rename = "restapi")]
    RestAPI(Option<AppConfig>),
    #[serde(rename = "http")]
    HTTP(Option<AppConfig>),
    #[serde(rename = "mq-api")]
    MQ(Option<AppConfig>),
    #[serde(rename = "embeddings")]
    Embeddings(Option<AppConfig>),
    #[serde(rename = "pganalyze")]
    PgAnalyze(Option<AppConfig>),
    #[serde(rename = "custom")]
    Custom(AppService),
}

// public facing interface for supported appService modes
// i.e. these are the configs end users can override in any given "App"
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct AppConfig {
    pub env: Option<Vec<EnvVar>>,
    pub resources: Option<ResourceRequirements>,
}

impl TryFrom<AppService> for AppType {
    type Error = &'static str;
    fn try_from(app_service: AppService) -> Result<Self, Self::Error> {
        let mut env: Option<Vec<EnvVar>> = None;
        if let Some(app_env) = app_service.env.as_ref() {
            env = Some(app_env.clone());
        }

        let resources = app_service.resources.clone();

        let app_config = Some(AppConfig {
            env,
            resources: Some(resources),
        });

        match app_service.name.as_str() {
            "ai-proxy" => Ok(AppType::RestAPI(app_config)),
            "restapi" => Ok(AppType::RestAPI(app_config)),
            "http" => Ok(AppType::HTTP(app_config)),
            "mq-api" => Ok(AppType::MQ(app_config)),
            "embeddings" => Ok(AppType::Embeddings(app_config)),
            "pganalyze" => Ok(AppType::PgAnalyze(app_config)),
            _ => {
                // everything else is a custom app
                Ok(AppType::Custom(app_service))
            }
        }
    }
}
