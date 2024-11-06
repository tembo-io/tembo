use crate::{
    apis::postgres_parameters::{
        merge_pg_configs, ConfigValue, MergeError, PgConfig, DISALLOWED_CONFIGS, MULTI_VAL_CONFIGS,
    },
    app_service::types::AppService,
    defaults,
    extensions::types::{Extension, ExtensionStatus, TrunkInstall, TrunkInstallStatus},
    postgres_exporter::PostgresMetrics,
};

use k8s_openapi::{
    api::core::v1::ResourceRequirements,
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::ObjectMeta},
};

use crate::cloudnativepg::clusters::{ClusterAffinity, ClusterTopologySpreadConstraints};
use crate::cloudnativepg::poolers::{
    PoolerPgbouncerPoolMode, PoolerTemplateSpecContainersResources,
};
use chrono::{DateTime, Utc};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tracing::error;
use utoipa::ToSchema;

const TLS_MIN_VERSION: &str = "TLSv1.2";

/// Stack type defines the stack configuration for the CoreDB instance.  This is
/// mainly used for the [https://tembo.io](https://tembo.io) platform to allow
/// for the deployment of pre-configured Postgres instances.
///
/// Standard, OLAP and the MessageQueue stacks are some of the common stacks configured
///
/// **Example**: Deploy a OLAP stack
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db
/// spec:
/// stack:
///   name: OLAP
///   postgres_config:
///     - name: checkpoint_timeout
///       value: "30min"
///     - name: pg_stat_statements.track
///       value: all
///     - name: track_io_timing
///       value: 'on'
///     - name: cron.host
///       value: /controller/run
///     - name: shared_preload_libraries
///       value: pg_stat_statements,pg_cron
/// ```
#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Stack {
    /// The name of the stack to deploy.
    pub name: String,

    /// The specific Postgres configuration settings needed for the stack.
    pub postgres_config: Option<Vec<PgConfig>>,
}

/// The ServiceAccountTemplate contains the template metadata needed to generate
/// the service accounts to be used by the underlying Postgres instance
///
/// For more information on service accounts please see the [Kubernetes documentation](https://kubernetes.io/docs/tasks/configure-pod-container/configure-service-account/)
/// and the Cloudnative-PG docs on [ServiceAccountTemplates](https://cloudnative-pg.io/documentation/1.20/cloudnative-pg.v1/#postgresql-cnpg-io-v1-ServiceAccountTemplate)
///
/// **Example**:
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///  name: test-db
/// spec:
///   serviceAccountTemplate:
///     metadata:
///       annotations:
///         eks.amazonaws.com/role-arn: arn:aws:iam::123456789012:role/pod-eks-role
/// ```
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct ServiceAccountTemplate {
    /// Metadata are the metadata to be used for the generated service account (Optional)
    pub metadata: Option<ObjectMeta>,
}

/// S3Credentials is the type for the credentials to be used to upload files to S3.
/// It can be provided in two alternative ways:
/// * explicitly passing accessKeyId and secretAccessKey
/// * inheriting the role from the pod environment by setting inheritFromIAMRole to true
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct S3Credentials {
    /// The reference to the access key id
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "accessKeyId"
    )]
    pub access_key_id: Option<S3CredentialsAccessKeyId>,

    /// Use the role based authentication without providing explicitly the keys.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "inheritFromIAMRole"
    )]
    pub inherit_from_iam_role: Option<bool>,

    /// The reference to the secret containing the region name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<S3CredentialsRegion>,

    /// The reference to the secret access key
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "secretAccessKey"
    )]
    pub secret_access_key: Option<S3CredentialsSecretAccessKey>,

    /// The references to the session key
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "sessionToken"
    )]
    pub session_token: Option<S3CredentialsSessionToken>,
}

/// S3CredentialsAccessKeyId is the type for the reference to the access key id
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct S3CredentialsAccessKeyId {
    pub key: String,
    pub name: String,
}

/// S3CredentialsRegion is the type for the reference to the secret containing the region name
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct S3CredentialsRegion {
    pub key: String,
    pub name: String,
}

/// S3CredentialsSecretAccessKey is the type for the reference to the secret access key
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct S3CredentialsSecretAccessKey {
    pub key: String,
    pub name: String,
}

/// S3CredentialsSessionToken is the type for the reference to the session key
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct S3CredentialsSessionToken {
    pub key: String,
    pub name: String,
}

/// GoogleCredentials is the type for the credentials to be used to upload files to Google Cloud Storage.
/// It can be provided in two alternative ways:
/// * The secret containing the Google Cloud Storage JSON file with the credentials (applicationCredentials)
/// * inheriting the role from the pod (GKE) environment by setting gkeEnvironment to true
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct GoogleCredentials {
    /// The reference to the secret containing the Google Cloud Storage JSON file with the credentials
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "applicationCredentials"
    )]
    pub application_credentials: Option<GoogleCredentialsApplicationCredentials>,

    /// Use the role based authentication without providing explicitly the keys.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "gkeEnvironment"
    )]
    pub gke_environment: Option<bool>,
}

/// GoogleCredentialsApplicationCredentials is the type for the reference to the secret containing the Google Cloud Storage JSON file with the credentials
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct GoogleCredentialsApplicationCredentials {
    pub key: String,
    pub name: String,
}

/// AzureCredentials is the type for the credentials to be used to upload files to Azure Blob Storage.
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct AzureCredentials {
    /// The connection string to be used
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "connectionString"
    )]
    pub connection_string: Option<AzureCredentialsConnectionString>,
    /// Use the Azure AD based authentication without providing explicitly the keys.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "inheritFromAzureAD"
    )]
    pub inherit_from_azure_ad: Option<bool>,
    /// The storage account where to upload data
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "storageAccount"
    )]
    pub storage_account: Option<AzureCredentialsStorageAccount>,
    /// The storage account key to be used in conjunction with the storage account name
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "storageKey"
    )]
    pub storage_key: Option<AzureCredentialsStorageKey>,
    /// A shared-access-signature to be used in conjunction with the storage account name
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "storageSasToken"
    )]
    pub storage_sas_token: Option<AzureCredentialsStorageSasToken>,
}

/// The connection string to be used for Azure Blob Storage backups
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct AzureCredentialsConnectionString {
    /// The key to select
    pub key: String,
    /// Name of the referent.
    pub name: String,
}

/// The storage account for Azure Blob Storage backups
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct AzureCredentialsStorageAccount {
    /// The key to select
    pub key: String,
    /// Name of the referent.
    pub name: String,
}

/// The storage account key to be used in conjunction with the storage account name for Azure Blob
/// Storage backups
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct AzureCredentialsStorageKey {
    /// The key to select
    pub key: String,
    /// Name of the referent.
    pub name: String,
}

/// A shared-access-signature to be used in conjunction with the storage account name for Azure Blob
/// Storage backups
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct AzureCredentialsStorageSasToken {
    /// The key to select
    pub key: String,
    /// Name of the referent.
    pub name: String,
}

/// VolumeSnapshots is the type for the configuration of the volume snapshots
/// to be used for backups instead of object storage
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema, PartialEq)]
pub struct VolumeSnapshot {
    /// Enable the volume snapshots for backups
    pub enabled: bool,

    /// The reference to the snapshot class
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "snapshotClass"
    )]
    pub snapshot_class: Option<String>,
}

/// CoreDB Backup configuration
/// The backup configuration for the CoreDB instance to facilitate database
/// backups uploads to an S3 compatible object store or using Volume Snapshots
/// For WAL archive uploads utilite an S3 compatible object store.
///
/// **Example**: A typical S3 backup configuration using IAM Role for authentication
/// with Volume Snapshots enabled
///
/// See `ServiceAccountTemplate` for to map the IAM role ARN to a Kubernetes service account.
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///  name: test-db
/// spec:
///   backup:
///     destinationPath: s3://my-bucket/my-backups
///     encryption: AES256
///     retentionPolicy: "30" #30 days
///     s3Credentials:
///       inheritFromIAMRole: true
///     schedule: "0 0 * * *" #every day at midnight
///     volumeSnapshots:
///       enabled: true
///       snapshotClass: my-snapshot-class-name
/// ```
#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
#[allow(non_snake_case)]
pub struct Backup {
    /// The S3 bucket path to store backups in
    #[serde(default = "defaults::default_destination_path")]
    pub destinationPath: Option<String>,

    /// The S3 encryption algorithm to use for backups
    #[serde(default = "defaults::default_encryption")]
    pub encryption: Option<String>,

    /// The number of days to retain backups for
    #[serde(default = "defaults::default_retention_policy")]
    pub retentionPolicy: Option<String>,

    /// The backup schedule set with cron syntax
    #[serde(default = "defaults::default_backup_schedule")]
    pub schedule: Option<String>,

    /// The S3 compatible endpoint URL
    #[serde(default, rename = "endpointURL")]
    pub endpoint_url: Option<String>,

    /// The S3 credentials to use for backups (if not using IAM Role)
    #[serde(rename = "s3Credentials")]
    pub s3_credentials: Option<S3Credentials>,

    /// The Google Cloud credentials to use for backups
    #[serde(rename = "googleCredentials")]
    pub google_credentials: Option<GoogleCredentials>,

    /// The Azure credentials to use for backups
    #[serde(rename = "azureCredentials")]
    pub azure_credentials: Option<AzureCredentials>,

    /// Enable using Volume Snapshots for backups instead of Object Storage
    #[serde(
        default = "defaults::default_volume_snapshot",
        rename = "volumeSnapshot"
    )]
    pub volume_snapshot: Option<VolumeSnapshot>,
}

/// Restore configuration provides a way to restore a database from a backup
/// stored in an S3 compatible object store.
///
/// **Example**: A typical S3 restore configuration using IAM Role for authentication
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db-restore
/// spec:
///   restore:
///     serverName: test-db
///     s3Credentials:
///       inheritFromIAMRole: true
/// ```
///
/// For more information please read through the [cloudnative-pg documentation](https://cloudnative-pg.io/documentation/1.20/recovery/#pitr-from-an-object-store)
#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
pub struct Restore {
    /// The name of the instance you wish to restore.  This maps to the `Backup`
    /// `destinationPath` field for the original instance.
    ///
    /// **Example**: If you have an instance with `spec.backup.destinationPath`
    /// set to `s3://my-bucket/test-db` then you would set `serverName` to `test-db`.
    ///
    /// This assumes you are keeping the backups in the new instance in the same
    /// root bucket path of `s3://my-bucket/`.
    #[serde(rename = "serverName")]
    pub server_name: String,

    /// The object storage path and bucket name of the instance you wish to restore from.  This maps to the `Backup`
    /// `destinationPath` field for the original instance.
    ///
    /// **Example**: If you have an instance with `spec.backup.destinationPath`
    /// set to `s3://my-bucket/v2/test-db` then you would set `backupsPath` to `s3://my-bucket/v2/test-db`.
    /// And backups are saved in that bucket under `s3://my-bucket/v2/test-db/server_name`
    #[serde(rename = "backupsPath")]
    pub backups_path: Option<String>,

    /// recovery_target_time is the time base target for point-in-time recovery.
    #[serde(rename = "recoveryTargetTime")]
    pub recovery_target_time: Option<String>,

    /// endpointURL is the S3 compatible endpoint URL
    #[serde(default, rename = "endpointURL")]
    pub endpoint_url: Option<String>,

    /// s3Credentials is the S3 credentials to use for restores.
    #[serde(rename = "s3Credentials")]
    pub s3_credentials: Option<S3Credentials>,

    /// googleCredentials is the Google Cloud credentials to use for restores.
    #[serde(rename = "googleCredentials")]
    pub google_credentials: Option<GoogleCredentials>,

    /// azureCredentials is the Azure credentials to use for restores.
    #[serde(rename = "azureCredentials")]
    pub azure_credentials: Option<AzureCredentials>,

    /// volumeSnapshot is a boolean to enable restoring from a Volume Snapshot
    #[serde(rename = "volumeSnapshot")]
    pub volume_snapshot: Option<bool>,
}

/// A connection pooler is a tool used to manage database connections, sitting
/// between your application and Postgres instance. Because of the way Postgres
/// handles connections, the server may encounter resource constraint issues
/// when managing a few thousand connections. Using a pooler can alleviate these
/// issues by using actual Postgres connections only when necessary
///
/// **Example**: A typical connection pooler configuration
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db
/// spec:
///   connectionPooler:
///     enabled: true
///     pooler:
///       poolMode: transaction
///       # Valid parameter values can be found at https://www.pgbouncer.org/config.html
///       parameters:
///         default_pool_size: "50"
///         max_client_conn: "5000"
///       resources:
///         limits:
///           cpu: 200m
///           memory: 256Mi
///         requests:
///           cpu: 100m
///           memory: 128Mi
/// ```
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, ToSchema, Default)]
#[allow(non_snake_case)]
pub struct ConnectionPooler {
    /// Enable the connection pooler
    ///
    /// **Default**: false.
    #[serde(default = "defaults::default_conn_pooler_enabled")]
    pub enabled: bool,

    /// The PGBouncer pooler configuration
    #[serde(default = "defaults::default_pgbouncer")]
    pub pooler: PgBouncer,
}

/// PgBouncer is the type for the PGBouncer configuration
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, ToSchema, Default)]
#[allow(non_snake_case)]
pub struct PgBouncer {
    /// The pool mode to use for the PGBouncer instance
    /// Specifies when a server connection can be reused by other clients.
    ///
    /// Valid values are:
    /// **session**: Server is released back to pool after client disconnects. Default.
    /// **transaction**: Server is released back to pool after transaction finishes.
    /// **statement**: Server is released back to pool after query finishes.
    /// Transactions spanning multiple statements are disallowed in this mode.
    ///
    /// **Default**: transaction
    #[serde(default = "defaults::default_pool_mode")]
    pub poolMode: PoolerPgbouncerPoolMode,

    /// Valid pgbouncer parameter values can be found at [https://www.pgbouncer.org/config.html](https://www.pgbouncer.org/config.html)
    pub parameters: Option<BTreeMap<String, String>>,

    /// The resource requirements (CPU/Memory) for the PGBouncer instance.
    /// This is the same format as what is set for a Kubernetes Pod.
    /// See [https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/](https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/)
    pub resources: Option<PoolerTemplateSpecContainersResources>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
#[allow(non_snake_case)]
pub struct DedicatedNetworking {
    /// Enable dedicated networking for the CoreDB instance.
    ///
    /// **Default**: false.
    #[serde(default)]
    pub enabled: bool,

    /// Include a separate load balancer for the standby (replica) server.
    ///
    /// **Default**: false.
    #[serde(default, rename = "includeStandby")]
    pub include_standby: bool,

    /// Configure the load balancer to be public or private.
    ///
    /// **Default**: true.
    #[serde(default)]
    pub public: bool,

    /// The type of Kubernetes Service to create (LoadBalancer or ClusterIP).
    ///
    /// **Default**: LoadBalancer.
    #[serde(default = "defaults::default_service_type")]
    pub serviceType: String,
}

impl DedicatedNetworking {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Generate the Kubernetes wrapper struct `CoreDB` from our Spec and Status struct
///
/// This provides a hook for generating the CRD yaml (in crdgen.rs)

/// CoreDBSpec represents the specification for a CoreDB instance. It defines
/// various configuration options for deploying and managing the database.
/// with the tembo-controller
///
/// # Basic CoreDB Configuration
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db
/// spec: {}
/// ````
#[derive(CustomResource, Default, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(kind = "CoreDB", group = "coredb.io", version = "v1alpha1", namespaced)]
#[kube(status = "CoreDBStatus", shortname = "cdb")]
#[allow(non_snake_case)]
pub struct CoreDBSpec {
    /// Number of CoreDB replicas to deploy.
    ///
    /// **Default**: 1.
    ///
    #[serde(default = "defaults::default_replicas")]
    pub replicas: i32,

    /// The resource requirements (CPU/Memory) for the CoreDB instance.
    /// This is the same format as what is set for a Kubernetes Pod.
    /// See [https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/](https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/)
    ///
    /// **Limits Default**: 2 CPU and 2Gi memory.
    ///
    /// **Requests Default**: 500m CPU and 512Mi memory.
    ///
    #[serde(default = "defaults::default_resources")]
    pub resources: ResourceRequirements,

    // The storage size for the Postgres data volume (PGDATA).
    //
    // **Default**: 8Gi.
    #[serde(default = "defaults::default_storage")]
    pub storage: Quantity,

    /// **DEPRECATED** The storage size for the sharedir volume.
    /// This is no longer used and will be removed in a future release.
    #[serde(default = "defaults::default_sharedir_storage")]
    pub sharedirStorage: Quantity,

    /// **DEPRECATED** The storage size for the pkglibdir volume.
    /// This is no longer used and will be removed in a future release.
    #[serde(default = "defaults::default_pkglibdir_storage")]
    pub pkglibdirStorage: Quantity,

    /// **DEPRECATED** Enable the use of the Postgres Exporter deployment for metrics collection
    /// This is no longer used and will be removed in a future release.
    #[serde(default = "defaults::default_postgres_exporter_enabled")]
    pub postgresExporterEnabled: bool,

    /// The Postgres image to use for the CoreDB instance deployment.
    /// This should be a valid Postgres image that is compatible with the
    /// [https://tembo.io](https://tembo.io) platform. For more information
    /// please visit our [tembo-images](https://github.com/tembo-io/tembo-images) repository.
    ///
    /// **Default**: quay.io/tembo/standard-cnpg:15.3.0-1-0c19c7e
    #[serde(default = "defaults::default_image_uri")]
    pub image: String,

    /// **DEPRECATED** The postgres-exporter image you want to use for the postgres-exporter deployment.
    /// This is no longer used and will be removed in a future release.
    #[serde(default = "defaults::default_postgres_exporter_image")]
    pub postgresExporterImage: String,

    /// Configuration for dedicated networking.
    ///
    /// **Default**: disabled
    #[serde(default, rename = "dedicatedNetworking")]
    pub dedicated_networking: Option<DedicatedNetworking>,

    /// The port to expose the Postgres service on.
    ///
    /// **Default**: 5432.
    #[serde(default = "defaults::default_port")]
    pub port: i32,

    /// **DEPRECATED** The UID to run the Postgres container as.
    /// This is no longer used and will be removed in a future release.
    ///
    /// We currently run the Postgres container with UID 26.
    #[serde(default = "defaults::default_uid")]
    pub uid: i32,

    /// A list of extensions to enable on the CoreDB instance.
    /// This list should be a list of extension names that are already available
    /// on the Postgres instance you are running.  To install extensions at runtime
    /// please see the `trunk_installs` field.
    ///
    /// **Default**: []
    #[serde(default = "defaults::default_extensions")]
    pub extensions: Vec<Extension>,

    /// A list of extensions to install from the [pgtrunk](https://pgt.dev) registry.
    /// This list should be a list of extension names and versions that you wish to
    /// install at runtime using the pgtrunk API.
    ///
    /// **Default**: []
    #[serde(default = "defaults::default_trunk_installs")]
    pub trunk_installs: Vec<TrunkInstall>,

    /// This option allows you to stop the database instance.
    ///
    /// **Default**: false.
    #[serde(default = "defaults::default_stop")]
    pub stop: bool,

    /// The serviceAccountTemplate contains the template needed to generate
    /// the service accounts to be used by the underlying Postgres instance
    ///
    /// For more information on service accounts please see the [Kubernetes documentation](https://kubernetes.io/docs/tasks/configure-pod-container/configure-service-account/)
    /// and the cloudnative-pg docs on [ServiceAccountTemplates](https://cloudnative-pg.io/documentation/1.20/cloudnative-pg.v1/#postgresql-cnpg-io-v1-ServiceAccountTemplate)
    ///
    /// **Default**: `ServiceAccountTemplate { metadata: None }`
    #[serde(default = "defaults::default_service_account_template")]
    pub serviceAccountTemplate: ServiceAccountTemplate,

    /// The backup configuration for the CoreDB instance to facilitate database
    /// backups and WAL archive uploads to an S3 compatible object store.
    ///
    /// **Default**: disabled
    #[serde(default = "defaults::default_backup")]
    pub backup: Backup,

    /// The metrics configuration to allow for custom Postgres metrics to be
    /// exposed in postgres-exporter and Prometheus.
    ///
    /// **Default**: disabled
    pub metrics: Option<PostgresMetrics>,

    /// The list of domains to add to the IngressRouteTCP generated in the
    /// tembo-controller to route traffic to the Postgres instance using SNI
    /// based routing of encrypted TLS traffic into the correct instance.
    ///
    /// **Default**: disabled
    pub extra_domains_rw: Option<Vec<String>>,

    /// List of IPv4 CIDR blocks to allow access to the Postgres instance.
    ///
    /// **Default**: Allow all
    #[serde(rename = "ipAllowList")]
    pub ip_allow_list: Option<Vec<String>>,

    /// The stack configuration for the CoreDB instance.  This is mainly used for the
    /// [https://tembo.io](https://tembo.io) platform to allow for the deployment of
    /// pre-configured Postgres instances.
    pub stack: Option<Stack>,

    /// The runtime_config is a way to set the Postgres configuration at runtime.
    /// This is a list of PgConfig objects that define the Postgres configuration
    ///
    /// For more information on what you can set, please refer to the cloudnative-pg
    /// documentation on setting [Postgres Parameters](https://cloudnative-pg.io/documentation/1.20/postgresql_conf/#postgresql-configuration)
    ///
    /// **Default**: disabled
    pub runtime_config: Option<Vec<PgConfig>>,

    /// The override_configs configuration is typically used by the [https://cloud.tembo.io](https://cloud.tembo.io)
    /// platform to allow the user to override the Postgres configuration at runtime.
    ///
    /// **Default**: disabled
    pub override_configs: Option<Vec<PgConfig>>,

    /// Connection pooler configuration used to manage database connections,
    /// sitting between your application and Postgres instance.  Currently when
    /// configured this will configure a PgBouncer instance in the namespace
    /// of your deployment
    ///
    /// **Default**: disabled
    #[serde(default = "defaults::default_conn_pooler")]
    pub connectionPooler: ConnectionPooler,

    /// app_service is a way to define a service that is deployed alongside the
    /// Postgres instance.  This is typically used to deploy a service that
    /// is used to connect to the Postgres instance in some manner.
    ///
    /// **Default**: disabled
    #[serde(rename = "appServices")]
    pub app_services: Option<Vec<AppService>>,

    /// The restore configuration provides a way to restore a database from a backup
    /// stored in an S3 compatible object store.
    ///
    /// **Default**: disabled
    pub restore: Option<Restore>,

    /// A StorageClass provides a way to describe the "classes" of storage offered
    /// in a cluster, including their provisioning, replication, and durability.
    ///
    /// For more information on StorageClasses please see the [Kubernetes documentation](https://kubernetes.io/docs/concepts/storage/storage-classes/)
    ///
    /// **Default**: `None` (uses the `default` StorageClass in your cluster)
    #[serde(rename = "storageClass")]
    pub storage_class: Option<String>,

    /// A AffinityConfiguration provides a way to configure the CoreDB instance to run
    /// on specific nodes in the cluster based off of nodeSelector, nodeAffinity and tolerations
    ///
    /// For more information on AffinityConfiguration please see the [Cloudnative-PG documentation](https://cloudnative-pg.io/documentation/1.22/cloudnative-pg.v1/#postgresql-cnpg-io-v1-AffinityConfiguration)
    ///
    /// **Default**:
    /// ```yaml
    /// apiVersion: coredb.io/v1alpha1
    /// kind: CoreDB
    /// metadata:
    ///   name: test-db-restore
    /// spec:
    ///   affinityConfiguration:
    ///     podAntiAffinityType: preferred
    ///     topologyKey: topology.kubernetes.io/zone
    /// ```
    #[serde(
        rename = "affinityConfiguration",
        default = "defaults::default_affinity_configuration"
    )]
    pub affinity_configuration: Option<ClusterAffinity>,

    /// The topologySpreadConstraints provides a way to spread matching pods among the given topology
    ///
    /// For more information see the Kubernetes documentation on [Topology Spread Constraints](https://kubernetes.io/docs/concepts/scheduling-eviction/topology-spread-constraints/)
    /// Tembo is compatible with the `v1` version of the TopologySpreadConstraints up to [Kubernetes 1.28](https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.25/#topologyspreadconstraint-v1-core)
    ///
    /// **Default**: `None`
    #[serde(rename = "topologySpreadConstraints")]
    pub topology_spread_constraints: Option<Vec<ClusterTopologySpreadConstraints>>,
}

impl CoreDBSpec {
    // extracts all Postgres configurations
    // configs can be defined in several different places (from a stack, user override, from an extension installation, user overrides, etc)
    pub fn get_pg_configs(
        &self,
        requires_load: BTreeMap<String, String>,
    ) -> Result<Option<Vec<PgConfig>>, MergeError> {
        let stack_configs = self
            .stack
            .as_ref()
            .and_then(|s| s.postgres_config.clone())
            .unwrap_or_default();
        let mut runtime_configs = self.runtime_config.clone().unwrap_or_default();
        // TODO: configs that come with extension installation
        // e.g. let extension_configs = ...
        // these extensions could be set by the operator, or trunk + operator
        // trunk install pg_partman could come with something like `pg_partman_bgw.dbname = xxx`

        // Get list of extension names that require load
        let mut include_with_shared_preload_libraries = BTreeSet::new();
        for ext in self.extensions.iter() {
            'loc: for location in ext.locations.iter() {
                if location.enabled && requires_load.contains_key(&ext.name) {
                    if let Some(library_name) = requires_load.get(&ext.name) {
                        include_with_shared_preload_libraries.insert(library_name.clone());
                    } else {
                        // coredb name not in scope, so can't be included in log
                        error!(
                            "Extension {} requires load but no library name was found",
                            ext.name
                        );
                    }
                    break 'loc;
                }
            }
        }

        let shared_preload_from_extensions =
            ConfigValue::Multiple(include_with_shared_preload_libraries);
        let extension_settings_config = vec![PgConfig {
            name: "shared_preload_libraries".to_string(),
            value: shared_preload_from_extensions,
        }];

        match merge_pg_configs(
            &runtime_configs,
            &extension_settings_config,
            "shared_preload_libraries",
        )? {
            None => {}
            Some(new_shared_preload_libraries) => {
                // check by name attribute if runtime_configs already has shared_preload_libraries
                // if so replace the value. Otherwise add this PgConfig into the vector.
                let mut found = false;
                for cfg in &mut runtime_configs {
                    if cfg.name == "shared_preload_libraries" {
                        cfg.value = new_shared_preload_libraries.value.clone();
                        found = true;
                        break;
                    }
                }
                if !found {
                    runtime_configs.push(new_shared_preload_libraries);
                }
            }
        }

        // handle merge of any of the settings that are multi-value.
        // e.g. stack defines shared_preload_libraries = pg_cron, then operator installs pg_stat_statements at runtime
        // we need to merge the two configs into one,  shared_preload_libraries = pg_cron, pg_stat_statements
        let mut merged_multival_configs: Vec<PgConfig> = Vec::new();
        for cfg_name in MULTI_VAL_CONFIGS {
            let merged_config = merge_pg_configs(&stack_configs, &runtime_configs, cfg_name)?;
            if let Some(merged_config) = merged_config {
                merged_multival_configs.push(merged_config);
            }
        }

        let default_settings = vec![PgConfig {
            name: "ssl_min_protocol_version".to_owned(),
            value: ConfigValue::Single(TLS_MIN_VERSION.to_string()),
        }];

        // Order matters - to ensure anything down stream does not have to worry about ordering,
        // set these into a BTreeSet now
        // 1. stack configs
        // 2. runtime configs
        // 3. merged multivals
        // 4. overrides
        let mut pg_configs: BTreeMap<String, PgConfig> = BTreeMap::new();

        for p in default_settings {
            pg_configs.insert(p.name.clone(), p);
        }
        for p in stack_configs {
            pg_configs.insert(p.name.clone(), p);
        }
        for p in runtime_configs {
            pg_configs.insert(p.name.clone(), p);
        }
        for p in merged_multival_configs {
            pg_configs.insert(p.name.clone(), p);
        }
        if let Some(override_configs) = &self.override_configs {
            for p in override_configs {
                pg_configs.insert(p.name.clone(), p.clone());
            }
        }

        // remove any configs that are not allowed
        for key in DISALLOWED_CONFIGS {
            pg_configs.remove(key);
        }

        if pg_configs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(pg_configs.values().cloned().collect()))
        }
    }

    pub fn get_pg_config_by_name(
        &self,
        config_name: &str,
        requires_load: BTreeMap<String, String>,
    ) -> Result<Option<PgConfig>, MergeError> {
        let all_configs = self.get_pg_configs(requires_load)?;
        for config in all_configs.unwrap_or_default() {
            if config.name == config_name {
                return Ok(Some(config));
            }
        }
        Ok(None)
    }
}

/// The status object of `CoreDB`
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[allow(non_snake_case)]
pub struct CoreDBStatus {
    pub running: bool,
    #[serde(default = "defaults::default_extensions_updating")]
    pub extensionsUpdating: bool,
    pub extensions: Option<Vec<ExtensionStatus>>,
    pub trunk_installs: Option<Vec<TrunkInstallStatus>>,
    pub storage: Option<Quantity>,
    pub resources: Option<ResourceRequirements>,
    pub runtime_config: Option<Vec<PgConfig>>,
    pub first_recoverability_time: Option<DateTime<Utc>>,
    pub pg_postmaster_start_time: Option<DateTime<Utc>>,
    #[deprecated(note = "This field is deprecated and it is no longer used")]
    pub last_fully_reconciled_at: Option<DateTime<Utc>>,
    pub last_archiver_status: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_core_db_spec() {
        let json_str = r#"
        {
          "image": "quay.io/tembo/tembo-pg-cnpg:15.3.0-5-cede445",
          "stack": {
            "name": "MessageQueue",
            "image": "quay.io/tembo/tembo-pg-cnpg:15.3.0-5-cede445",
            "services": null,
            "extensions": [
              {
                "name": "pgmq",
                "locations": [
                  {
                    "schema": null,
                    "enabled": true,
                    "version": "0.10.2",
                    "database": "postgres"
                  }
                ],
                "description": null
              },
              {
                "name": "pg_partman",
                "locations": [
                  {
                    "schema": null,
                    "enabled": true,
                    "version": "4.7.3",
                    "database": "postgres"
                  }
                ],
                "description": null
              }
            ],
            "description": "A Tembo Postgres Stack optimized for Message Queue workloads.",
            "stack_version": "0.2.0",
            "infrastructure": {
              "cpu": "1",
              "memory": "1Gi",
              "region": "us-east-1",
              "provider": "aws",
              "storage_size": "10Gi",
              "instance_type": "GeneralPurpose",
              "storage_class": "gp3"
            },
            "trunk_installs": [
              {
                "name": "pgmq",
                "version": "0.10.2"
              },
              {
                "name": "pg_partman",
                "version": "4.7.3"
              }
            ],
            "postgres_config": [
              {
                "name": "shared_preload_libraries",
                "value": "pg_stat_statements,pg_partman_bgw"
              },
              {
                "name": "pg_partman_bgw.dbname",
                "value": "postgres"
              },
              {
                "name": "pg_partman_bgw.interval",
                "value": "60"
              },
              {
                "name": "pg_partman_bgw.role",
                "value": "postgres"
              },
              {
                "name": "random_page_cost",
                "value": "1.1"
              },
              {
                "name": "autovacuum_vacuum_cost_limit",
                "value": "-1"
              },
              {
                "name": "autovacuum_vacuum_scale_factor",
                "value": "0.05"
              },
              {
                "name": "autovacuum_vacuum_insert_scale_factor",
                "value": "0.05"
              },
              {
                "name": "autovacuum_analyze_scale_factor",
                "value": "0.05"
              },
              {
                "name": "checkpoint_timeout",
                "value": "10min"
              },
              {
                "name": "pg_stat_statements.track",
                "value": "all"
              }
            ],
            "postgres_metrics": {
              "pgmq": {
                "query": "select queue_name, queue_length, oldest_msg_age_sec, newest_msg_age_sec, total_messages from public.pgmq_metrics_all()",
                "master": true,
                "metrics": [
                  {
                    "queue_name": {
                      "usage": "LABEL",
                      "description": "Name of the queue"
                    }
                  },
                  {
                    "queue_length": {
                      "usage": "GAUGE",
                      "description": "Number of messages in the queue"
                    }
                  },
                  {
                    "oldest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the oldest message in the queue, in seconds."
                    }
                  },
                  {
                    "newest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the newest message in the queue, in seconds."
                    }
                  },
                  {
                    "total_messages": {
                      "usage": "GAUGE",
                      "description": "Total number of messages that have passed into the queue."
                    }
                  }
                ]
              }
            },
            "postgres_config_engine": "standard"
          },
          "metrics": {
            "image": "quay.io/prometheuscommunity/postgres-exporter:v0.12.0",
            "enabled": true,
            "queries": {
              "pgmq": {
                "query": "select queue_name, queue_length, oldest_msg_age_sec, newest_msg_age_sec, total_messages from public.pgmq_metrics_all()",
                "master": true,
                "metrics": [
                  {
                    "queue_name": {
                      "usage": "LABEL",
                      "description": "Name of the queue"
                    }
                  },
                  {
                    "queue_length": {
                      "usage": "GAUGE",
                      "description": "Number of messages in the queue"
                    }
                  },
                  {
                    "oldest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the oldest message in the queue, in seconds."
                    }
                  },
                  {
                    "newest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the newest message in the queue, in seconds."
                    }
                  },
                  {
                    "total_messages": {
                      "usage": "GAUGE",
                      "description": "Total number of messages that have passed into the queue."
                    }
                  }
                ]
              }
            }
          },
          "storage": "10Gi",
          "resources": {
            "limits": {
              "cpu": "1",
              "memory": "1Gi"
            }
          },
          "extensions": [
            {
              "name": "pgmq",
              "locations": [
                {
                  "schema": null,
                  "enabled": true,
                  "version": "0.10.2",
                  "database": "postgres"
                }
              ],
              "description": null
            },
            {
              "name": "pg_partman",
              "locations": [
                {
                  "schema": null,
                  "enabled": true,
                  "version": "4.7.3",
                  "database": "postgres"
                }
              ],
              "description": null
            }
          ],
          "runtime_config": [
            {
              "name": "shared_buffers",
              "value": "256MB"
            },
            {
              "name": "max_connections",
              "value": "107"
            },
            {
              "name": "work_mem",
              "value": "5MB"
            },
            {
              "name": "bgwriter_delay",
              "value": "200ms"
            },
            {
              "name": "effective_cache_size",
              "value": "716MB"
            },
            {
              "name": "maintenance_work_mem",
              "value": "64MB"
            },
            {
              "name": "max_wal_size",
              "value": "2GB"
            }
          ],
          "trunk_installs": [
            {
              "name": "pgmq",
              "version": "0.10.2"
            },
            {
              "name": "pg_partman",
              "version": "4.7.3"
            }
          ],
          "postgresExporterEnabled": true
        }
        "#;

        let _deserialized_spec: CoreDBSpec = serde_json::from_str(json_str).unwrap();
    }
}
