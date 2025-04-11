pub mod backups;
pub mod clusters;
pub(crate) mod cnpg;
// pub(crate) mod cnpg_backups;
pub(crate) mod archive;
pub mod cnpg_utils;
pub mod hibernate;
pub mod objectstore;
pub(crate) mod placement;
pub(crate) mod plugins;
pub mod poolers;
pub mod retention;
mod scheduledbackups;
pub const VOLUME_SNAPSHOT_CLASS_NAME: &str = "cnpg-snapshot-class";
pub const BARMAN_CLOUD_OBJECTSTORE_PLUGIN_NAME: &str = "barman-cloud.cloudnative-pg.io";

use crate::cloudnativepg::clusters::{
    ClusterBackupBarmanObjectStoreAzureCredentials,
    ClusterBackupBarmanObjectStoreGoogleCredentials, ClusterBackupBarmanObjectStoreS3Credentials,
};
use crate::cloudnativepg::plugins::objectstores::{
    ObjectStore, ObjectStoreConfigurationAzureCredentials,
    ObjectStoreConfigurationAzureCredentialsConnectionString,
    ObjectStoreConfigurationAzureCredentialsStorageAccount,
    ObjectStoreConfigurationAzureCredentialsStorageKey,
    ObjectStoreConfigurationAzureCredentialsStorageSasToken,
    ObjectStoreConfigurationGoogleCredentials,
    ObjectStoreConfigurationGoogleCredentialsApplicationCredentials,
    ObjectStoreConfigurationS3Credentials, ObjectStoreConfigurationS3CredentialsAccessKeyId,
    ObjectStoreConfigurationS3CredentialsRegion,
    ObjectStoreConfigurationS3CredentialsSecretAccessKey,
    ObjectStoreConfigurationS3CredentialsSessionToken, ObjectStoreSpec,
};

impl Default for ObjectStore {
    fn default() -> Self {
        ObjectStore {
            metadata: Default::default(),
            spec: ObjectStoreSpec {
                configuration: Default::default(),
                instance_sidecar_configuration: None,
                retention_policy: None,
            },
            status: None,
        }
    }
}

impl From<ClusterBackupBarmanObjectStoreS3Credentials> for ObjectStoreConfigurationS3Credentials {
    fn from(creds: ClusterBackupBarmanObjectStoreS3Credentials) -> Self {
        ObjectStoreConfigurationS3Credentials {
            access_key_id: creds.access_key_id.map(|id| {
                ObjectStoreConfigurationS3CredentialsAccessKeyId {
                    key: id.key,
                    name: id.name,
                }
            }),
            inherit_from_iam_role: creds.inherit_from_iam_role,
            region: creds
                .region
                .map(|r| ObjectStoreConfigurationS3CredentialsRegion {
                    key: r.key,
                    name: r.name,
                }),
            secret_access_key: creds.secret_access_key.map(|key| {
                ObjectStoreConfigurationS3CredentialsSecretAccessKey {
                    key: key.key,
                    name: key.name,
                }
            }),
            session_token: creds.session_token.map(|token| {
                ObjectStoreConfigurationS3CredentialsSessionToken {
                    key: token.key,
                    name: token.name,
                }
            }),
        }
    }
}

impl From<ClusterBackupBarmanObjectStoreGoogleCredentials>
    for ObjectStoreConfigurationGoogleCredentials
{
    fn from(creds: ClusterBackupBarmanObjectStoreGoogleCredentials) -> Self {
        ObjectStoreConfigurationGoogleCredentials {
            application_credentials: creds.application_credentials.map(|ac| {
                ObjectStoreConfigurationGoogleCredentialsApplicationCredentials {
                    key: ac.key,
                    name: ac.name,
                }
            }),
            gke_environment: creds.gke_environment,
        }
    }
}

impl From<ClusterBackupBarmanObjectStoreAzureCredentials>
    for ObjectStoreConfigurationAzureCredentials
{
    fn from(creds: ClusterBackupBarmanObjectStoreAzureCredentials) -> Self {
        ObjectStoreConfigurationAzureCredentials {
            connection_string: creds.connection_string.map(|cs| {
                ObjectStoreConfigurationAzureCredentialsConnectionString {
                    key: cs.key,
                    name: cs.name,
                }
            }),
            inherit_from_azure_ad: creds.inherit_from_azure_ad,
            storage_account: creds.storage_account.map(|sa| {
                ObjectStoreConfigurationAzureCredentialsStorageAccount {
                    key: sa.key,
                    name: sa.name,
                }
            }),
            storage_key: creds.storage_key.map(|sk| {
                ObjectStoreConfigurationAzureCredentialsStorageKey {
                    key: sk.key,
                    name: sk.name,
                }
            }),
            storage_sas_token: creds.storage_sas_token.map(|st| {
                ObjectStoreConfigurationAzureCredentialsStorageSasToken {
                    key: st.key,
                    name: st.name,
                }
            }),
        }
    }
}
