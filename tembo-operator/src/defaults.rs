use k8s_openapi::{
    api::core::v1::ResourceRequirements,
    apimachinery::pkg::{api::resource::Quantity, util::intstr::IntOrString},
};
use std::collections::BTreeMap;

use crate::{
    apis::coredb_types::{
        Backup, ConnectionPooler, PgBouncer, S3Credentials, ServiceAccountTemplate, VolumeSnapshot,
    },
    cloudnativepg::poolers::{PoolerPgbouncerPoolMode, PoolerTemplateSpecContainersResources},
    extensions::types::{Extension, TrunkInstall},
    stacks::{types::ImagePerPgVersion, DATAWAREHOUSE, ML, STANDARD},
};

pub fn default_replicas() -> i32 {
    1
}

pub fn default_resources() -> ResourceRequirements {
    let limits: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("2".to_string())),
        ("memory".to_owned(), Quantity("2Gi".to_string())),
    ]);
    let requests: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("500m".to_string())),
        ("memory".to_owned(), Quantity("512Mi".to_string())),
    ]);
    ResourceRequirements {
        limits: Some(limits),
        requests: Some(requests),
    }
}

pub fn default_postgres_exporter_enabled() -> bool {
    false
}

pub fn default_uid() -> i32 {
    999
}

pub fn default_port() -> i32 {
    5432
}

pub fn default_repository() -> String {
    "quay.io/tembo".to_owned()
}

pub fn default_images() -> ImagePerPgVersion {
    // Note: this will recurse infinitely if standard.yaml doesn't have `images` set
    STANDARD.images.clone()
}

pub fn default_llm_image() -> String {
    ML.images.pg15.clone()
}

pub fn default_dw_image() -> String {
    DATAWAREHOUSE.images.pg15.clone()
}

pub fn default_image_uri() -> String {
    let repo = default_repository();
    let image = default_images();
    let image_for_pg_15 = image.pg15;
    format!("{}/{}", repo, image_for_pg_15)
}

pub fn default_llm_image_uri() -> String {
    let repo = default_repository();
    let image = default_llm_image();
    format!("{}/{}", repo, image)
}

pub fn default_dw_image_uri() -> String {
    let repo = default_repository();
    let image = default_dw_image();
    format!("{}/{}", repo, image)
}

pub fn default_storage() -> Quantity {
    Quantity("8Gi".to_string())
}

pub fn default_sharedir_storage() -> Quantity {
    Quantity("1Gi".to_string())
}

pub fn default_pkglibdir_storage() -> Quantity {
    Quantity("1Gi".to_string())
}

pub fn default_postgres_exporter_image() -> String {
    "quay.io/prometheuscommunity/postgres-exporter:v0.12.0".to_owned()
}

pub fn default_postgres_exporter_target_databases() -> Vec<String> {
    vec!["postgres".to_owned()]
}

pub fn default_extensions() -> Vec<Extension> {
    vec![]
}

pub fn default_trunk_installs() -> Vec<TrunkInstall> {
    vec![]
}

pub fn default_database() -> String {
    "postgres".to_owned()
}

pub fn default_schema() -> String {
    "public".to_owned()
}

pub fn default_description() -> Option<String> {
    Some("No description provided".to_owned())
}

pub fn default_stop() -> bool {
    false
}

pub fn default_extensions_updating() -> bool {
    false
}

pub fn default_service_account_template() -> ServiceAccountTemplate {
    ServiceAccountTemplate { metadata: None }
}

pub fn default_backup() -> Backup {
    Backup {
        destinationPath: default_destination_path(),
        encryption: default_encryption(),
        retentionPolicy: default_retention_policy(),
        schedule: default_backup_schedule(),
        s3_credentials: default_s3_credentials(),
        volume_snapshot: default_volume_snapshot(),
        ..Default::default()
    }
}

pub fn default_destination_path() -> Option<String> {
    Some("s3://".to_string())
}

pub fn default_encryption() -> Option<String> {
    Some("AES256".to_owned())
}

pub fn default_retention_policy() -> Option<String> {
    Some("30".to_owned())
}

pub fn default_backup_schedule() -> Option<String> {
    // Every day at midnight
    Some("0 0 * * *".to_owned())
}

pub fn default_conn_pooler() -> ConnectionPooler {
    ConnectionPooler {
        enabled: default_conn_pooler_enabled(),
        pooler: default_pgbouncer(),
    }
}

pub fn default_conn_pooler_enabled() -> bool {
    false
}

pub fn default_pool_mode() -> PoolerPgbouncerPoolMode {
    PoolerPgbouncerPoolMode::Transaction
}

pub fn default_pooler_parameters() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("default_pool_size".to_string(), "50".to_string()),
        ("max_client_conn".to_string(), "5000".to_string()),
    ])
}

pub fn default_pooler_resources() -> PoolerTemplateSpecContainersResources {
    PoolerTemplateSpecContainersResources {
        claims: None,
        limits: Some(BTreeMap::from([
            ("cpu".to_owned(), IntOrString::String("100m".to_owned())),
            ("memory".to_owned(), IntOrString::String("128Mi".to_owned())),
        ])),
        requests: Some(BTreeMap::from([
            ("cpu".to_owned(), IntOrString::String("50m".to_owned())),
            ("memory".to_owned(), IntOrString::String("64Mi".to_owned())),
        ])),
    }
}

pub fn default_pgbouncer() -> PgBouncer {
    PgBouncer {
        poolMode: default_pool_mode(),
        parameters: Some(default_pooler_parameters()),
        resources: Some(default_pooler_resources()),
    }
}

pub fn default_s3_credentials() -> Option<S3Credentials> {
    Some(S3Credentials {
        inherit_from_iam_role: Some(true),
        ..Default::default()
    })
}

pub fn default_volume_snapshot() -> Option<VolumeSnapshot> {
    Some(VolumeSnapshot {
        enabled: false,
        snapshot_class: None,
    })
}
