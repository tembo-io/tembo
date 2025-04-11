use crate::{
    apis::coredb_types::CoreDB,
    cloudnativepg::{
        cnpg::{
            generate_azure_backup_credentials, generate_google_backup_credentials,
            generate_s3_backup_credentials,
        },
        plugins::objectstores::{
            ObjectStore, ObjectStoreConfiguration, ObjectStoreConfigurationAzureCredentials,
            ObjectStoreConfigurationData, ObjectStoreConfigurationDataCompression,
            ObjectStoreConfigurationDataEncryption, ObjectStoreConfigurationGoogleCredentials,
            ObjectStoreConfigurationS3Credentials, ObjectStoreConfigurationWal,
            ObjectStoreConfigurationWalCompression, ObjectStoreConfigurationWalEncryption,
            ObjectStoreSpec,
        },
        BARMAN_CLOUD_OBJECTSTORE_PLUGIN_NAME,
    },
    config::Config,
    Context,
};
use kube::{
    api::{Api, Patch, PatchParams, PostParams},
    core::ObjectMeta,
    runtime::controller::Action,
    ResourceExt,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, warn};

enum ObjectStoreCredentials {
    S3(ObjectStoreConfigurationS3Credentials),
    Google(ObjectStoreConfigurationGoogleCredentials),
    Azure(ObjectStoreConfigurationAzureCredentials),
}

fn create_object_store_config_data(cdb: &CoreDB) -> Option<ObjectStoreConfigurationData> {
    let encryption = match &cdb.spec.backup.encryption {
        Some(encryption) => match encryption.as_str() {
            "AES256" => Some(ObjectStoreConfigurationDataEncryption::Aes256),
            "aws:kms" => Some(ObjectStoreConfigurationDataEncryption::AwsKms),
            _ => None,
        },
        _ => None,
    };

    Some(ObjectStoreConfigurationData {
        compression: Some(ObjectStoreConfigurationDataCompression::Snappy),
        encryption,
        immediate_checkpoint: Some(true),
        ..ObjectStoreConfigurationData::default()
    })
}

fn create_object_store_wal(cdb: &CoreDB) -> Option<ObjectStoreConfigurationWal> {
    let encryption = match &cdb.spec.backup.encryption {
        Some(encryption) => match encryption.as_str() {
            "AES256" => Some(ObjectStoreConfigurationWalEncryption::Aes256),
            "aws:kms" => Some(ObjectStoreConfigurationWalEncryption::AwsKms),
            _ => None,
        },
        _ => None,
    };

    if encryption.is_some() {
        Some(ObjectStoreConfigurationWal {
            compression: Some(ObjectStoreConfigurationWalCompression::Snappy),
            encryption,
            max_parallel: Some(8),
            ..ObjectStoreConfigurationWal::default()
        })
    } else {
        None
    }
}

fn create_object_store_config(
    cdb: &CoreDB,
    endpoint_url: Option<String>,
    backup_path: &str,
    credentials: Option<ObjectStoreCredentials>,
) -> ObjectStoreConfiguration {
    // For backwards compatibility, default to inherited IAM role
    let credentials = credentials.unwrap_or(ObjectStoreCredentials::S3(
        ObjectStoreConfigurationS3Credentials {
            inherit_from_iam_role: Some(true),
            ..ObjectStoreConfigurationS3Credentials::default()
        },
    ));

    let mut object_store_config = ObjectStoreConfiguration {
        data: create_object_store_config_data(cdb),
        endpoint_url,
        destination_path: backup_path.to_string(),
        wal: create_object_store_wal(cdb),
        ..ObjectStoreConfiguration::default()
    };

    match credentials {
        ObjectStoreCredentials::S3(creds) => object_store_config.s3_credentials = Some(creds),
        ObjectStoreCredentials::Google(creds) => {
            object_store_config.google_credentials = Some(creds)
        }
        ObjectStoreCredentials::Azure(creds) => object_store_config.azure_credentials = Some(creds),
    }

    object_store_config
}

fn create_backup_object_store(
    cdb: &CoreDB,
    endpoint_url: Option<String>,
    backup_path: &str,
    credentials: Option<ObjectStoreCredentials>,
) -> ObjectStoreSpec {
    let retention_days = match &cdb.spec.backup.retentionPolicy {
        None => "30d".to_string(),
        Some(retention_policy) => match retention_policy.parse::<i32>() {
            Ok(days) => {
                format!("{}d", days)
            }
            Err(_) => {
                warn!("Invalid retention policy because could not convert to i32, using default of 30 days");
                "30d".to_string()
            }
        },
    };

    ObjectStoreSpec {
        configuration: create_object_store_config(cdb, endpoint_url, backup_path, credentials),
        retention_policy: Some(retention_days),
        ..ObjectStoreSpec::default()
    }
}

// create_object_store creates an ObjectStore CRD for the given CoreDB instance.
// This will manage a sidecar that runs in the instance pods to manage backups and WALs to an object store.
fn create_object_store(cdb: &CoreDB, cfg: &Config) -> Option<ObjectStore> {
    // do not reconcile if backup is not enabled
    if !cfg.enable_backup {
        return None;
    }
    let backup_path = cdb.spec.backup.destinationPath.clone();

    // Copy the endpoint_url and s3_credentials from cdb to configure backups
    let backup_credentials = if let Some(s3_creds) = cdb.spec.backup.s3_credentials.as_ref() {
        Some(ObjectStoreCredentials::S3(
            generate_s3_backup_credentials(Some(s3_creds)).into(),
        ))
    } else if let Some(gcs_creds) = cdb.spec.backup.google_credentials.as_ref() {
        generate_google_backup_credentials(Some(gcs_creds.clone()))
            .map(|creds| ObjectStoreCredentials::Google(creds.into()))
    } else if let Some(azure_creds) = cdb.spec.backup.azure_credentials.as_ref() {
        generate_azure_backup_credentials(Some(azure_creds.clone()))
            .map(|creds| ObjectStoreCredentials::Azure(creds.into()))
    } else {
        None
    };
    let object_store_plugin_name = BARMAN_CLOUD_OBJECTSTORE_PLUGIN_NAME.to_string();
    let object_store_plugin_config = ObjectStore {
        metadata: ObjectMeta {
            name: Some(object_store_plugin_name),
            namespace: Some(cdb.name_any()),
            ..Default::default()
        },
        spec: create_backup_object_store(
            cdb,
            cdb.spec.backup.endpoint_url.clone(),
            &backup_path.unwrap_or_default(),
            backup_credentials,
        ),
        status: None,
    };

    Some(object_store_plugin_config)
}

pub async fn reconcile_object_store(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    debug!("Creating object store");
    let cfg = Config::default();
    let object_store = create_object_store(cdb, &cfg).unwrap_or_default();
    let object_store_name = object_store.metadata.name.clone().unwrap_or_default();

    let object_store_api: Api<ObjectStore> = Api::namespaced(ctx.client.clone(), &cdb.name_any());
    let maybe_object_store = object_store_api.get(&object_store_name).await;

    match maybe_object_store {
        Ok(_current_object_store) => {
            // Object store exists, patch it
            let patch_params = PatchParams::apply("tembo-operator").force();
            match object_store_api
                .patch(
                    &object_store_name,
                    &patch_params,
                    &Patch::Apply(&object_store),
                )
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Failed to patch object store: {:?}", e);
                    Err(Action::requeue(Duration::from_secs(300)))
                }
            }
        }
        Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
            // Object store doesn't exist, create it
            match object_store_api
                .create(&PostParams::default(), &object_store)
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Failed to create object store: {:?}", e);
                    Err(Action::requeue(Duration::from_secs(300)))
                }
            }
        }
        Err(e) => {
            error!("Error getting object store: {:?}", e);
            Err(Action::requeue(Duration::from_secs(300)))
        }
    }
}
