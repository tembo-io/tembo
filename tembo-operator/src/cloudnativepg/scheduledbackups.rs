// WARNING: generated by kopium - manual changes will be overwritten
// kopium command: kopium -D Default scheduledbackups.postgresql.cnpg.io -A
// kopium version: 0.16.5

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Specification of the desired behavior of the ScheduledBackup. More info: https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#spec-and-status
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
#[kube(
    group = "postgresql.cnpg.io",
    version = "v1",
    kind = "ScheduledBackup",
    plural = "scheduledbackups"
)]
#[kube(namespaced)]
#[kube(status = "ScheduledBackupStatus")]
pub struct ScheduledBackupSpec {
    /// Indicates which ownerReference should be put inside the created backup resources.<br /> - none: no owner reference for created backup objects (same behavior as before the field was introduced)<br /> - self: sets the Scheduled backup object as owner of the backup<br /> - cluster: set the cluster as owner of the backup<br />
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "backupOwnerReference"
    )]
    pub backup_owner_reference: Option<ScheduledBackupBackupOwnerReference>,
    /// The cluster to backup
    pub cluster: ScheduledBackupCluster,
    /// If the first backup has to be immediately start after creation or not
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immediate: Option<bool>,
    /// The backup method to be used, possible options are `barmanObjectStore` and `volumeSnapshot`. Defaults to: `barmanObjectStore`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<ScheduledBackupMethod>,
    /// Whether the default type of backup with volume snapshots is online/hot (`true`, default) or offline/cold (`false`) Overrides the default setting specified in the cluster field '.spec.backup.volumeSnapshot.online'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,
    /// Configuration parameters to control the online/hot backup with volume snapshots Overrides the default settings specified in the cluster '.backup.volumeSnapshot.onlineConfiguration' stanza
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "onlineConfiguration"
    )]
    pub online_configuration: Option<ScheduledBackupOnlineConfiguration>,
    /// The schedule does not follow the same format used in Kubernetes CronJobs as it includes an additional seconds specifier, see https://pkg.go.dev/github.com/robfig/cron#hdr-CRON_Expression_Format
    pub schedule: String,
    /// If this backup is suspended or not
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspend: Option<bool>,
    /// The policy to decide which instance should perform this backup. If empty, it defaults to `cluster.spec.backup.target`. Available options are empty string, `primary` and `prefer-standby`. `primary` to have backups run always on primary instances, `prefer-standby` to have backups run preferably on the most updated standby, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ScheduledBackupTarget>,
}

/// Specification of the desired behavior of the ScheduledBackup. More info: https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#spec-and-status
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub enum ScheduledBackupBackupOwnerReference {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "self")]
    r#_Self,
    #[serde(rename = "cluster")]
    Cluster,
}

/// The cluster to backup
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct ScheduledBackupCluster {
    /// Name of the referent.
    pub name: String,
}

/// Specification of the desired behavior of the ScheduledBackup. More info: https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#spec-and-status
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
pub enum ScheduledBackupMethod {
    #[serde(rename = "barmanObjectStore")]
    BarmanObjectStore,
    #[serde(rename = "volumeSnapshot")]
    VolumeSnapshot,
}

/// Configuration parameters to control the online/hot backup with volume snapshots Overrides the default settings specified in the cluster '.backup.volumeSnapshot.onlineConfiguration' stanza
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct ScheduledBackupOnlineConfiguration {
    /// Control whether the I/O workload for the backup initial checkpoint will be limited, according to the `checkpoint_completion_target` setting on the PostgreSQL server. If set to true, an immediate checkpoint will be used, meaning PostgreSQL will complete the checkpoint as soon as possible. `false` by default.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "immediateCheckpoint"
    )]
    pub immediate_checkpoint: Option<bool>,
    /// If false, the function will return immediately after the backup is completed, without waiting for WAL to be archived. This behavior is only useful with backup software that independently monitors WAL archiving. Otherwise, WAL required to make the backup consistent might be missing and make the backup useless. By default, or when this parameter is true, pg_backup_stop will wait for WAL to be archived when archiving is enabled. On a standby, this means that it will wait only when archive_mode = always. If write activity on the primary is low, it may be useful to run pg_switch_wal on the primary in order to trigger an immediate segment switch.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "waitForArchive"
    )]
    pub wait_for_archive: Option<bool>,
}

/// Specification of the desired behavior of the ScheduledBackup. More info: https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#spec-and-status
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub enum ScheduledBackupTarget {
    #[serde(rename = "primary")]
    Primary,
    #[serde(rename = "prefer-standby")]
    PreferStandby,
}

/// Most recently observed status of the ScheduledBackup. This data may not be up to date. Populated by the system. Read-only. More info: https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#spec-and-status
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct ScheduledBackupStatus {
    /// The latest time the schedule
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "lastCheckTime"
    )]
    pub last_check_time: Option<String>,
    /// Information when was the last time that backup was successfully scheduled.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "lastScheduleTime"
    )]
    pub last_schedule_time: Option<String>,
    /// Next time we will run a backup
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "nextScheduleTime"
    )]
    pub next_schedule_time: Option<String>,
}
