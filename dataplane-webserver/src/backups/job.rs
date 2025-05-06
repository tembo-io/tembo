use crate::backups::types::JobStatus;
use actix_web::{error::ErrorInternalServerError, Error};
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::batch::v1::Job as K8sJob;
use k8s_openapi::api::core::v1::{
    Container, EnvFromSource, EphemeralVolumeSource, PersistentVolumeClaimSpec,
    PersistentVolumeClaimTemplate, PodSpec, PodTemplateSpec, SecretEnvSource, Toleration, Volume,
    VolumeMount, VolumeResourceRequirements,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::{
    api::{ObjectMeta, PostParams},
    Api, Client as KubeClient,
};
use std::collections::BTreeMap;
use tracing;

/// Creates a Kubernetes Job to run a temback backup in the given namespace.
///
/// The Job will:
/// - Use the `{namespace}-connection` secret via envFrom for connection info
/// - Mount a generic ephemeral volume (PersistentVolumeClaimTemplate) sized to the given storage_size using the 'gp3-enc' StorageClass
/// - Run the temback command, using env vars for connection info
/// - Use the provided temback_image
///
/// # Arguments
/// * `kube_client` - Kubernetes client
/// * `namespace` - Namespace to create the Job in
/// * `job_id` - Unique job identifier (used for Job name)
/// * `s3_bucket` - S3 bucket name
/// * `s3_path` - S3 object path
/// * `storage_size` - Size for the ephemeral volume (e.g., "10Gi")
/// * `temback_image` - Image to use for the temback job
///
/// # Returns
/// * `Ok(())` if the Job is created successfully
/// * `Err(Error)` if Job creation fails
pub async fn create_backup_job(
    kube_client: &KubeClient,
    namespace: &str,
    job_id: &str,
    s3_bucket: &str,
    s3_path: &str,
    storage_size: &str,
    temback_image: &str,
) -> Result<(), Error> {
    let job_name = format!("temback-backup-{}", job_id);
    let secret_name = format!("{namespace}-connection");
    let volume_name = "backup-tmp";
    let mount_path = "/backup";
    let host_rw = format!("{namespace}-rw.{namespace}.svc.cluster.local");
    let args = [
        "--name",
        "$(NAMESPACE)",
        "--host",
        "$(HOST_RW)",
        "--user",
        "$(user)",
        "--pass",
        "$(password)",
        "--bucket",
        "$(BUCKET)",
        "--dir",
        "$(BUCKET_PATH)",
        "--cd",
        "$(MOUNT_PATH)",
        "--compress",
        "--clean",
    ];

    let job = Job {
        metadata: ObjectMeta {
            name: Some(job_name.clone()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: Some(k8s_openapi::api::batch::v1::JobSpec {
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(
                        [
                            ("job-name".to_string(), job_name.clone()),
                            ("app".to_string(), "temback-backup".to_string()),
                            ("coredb.io/name".to_string(), namespace.to_string()),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "temback".to_string(),
                        image: Some(temback_image.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        args: Some(args.iter().map(|s| s.to_string()).collect()),
                        env_from: Some(vec![EnvFromSource {
                            secret_ref: Some(SecretEnvSource {
                                name: secret_name.clone(),
                                optional: Some(false),
                            }),
                            ..Default::default()
                        }]),
                        env: Some(vec![
                            k8s_openapi::api::core::v1::EnvVar {
                                name: "JOB_ID".to_string(),
                                value: Some(job_id.to_string()),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::EnvVar {
                                name: "BUCKET".to_string(),
                                value: Some(s3_bucket.to_string()),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::EnvVar {
                                name: "BUCKET_PATH".to_string(),
                                value: Some(s3_path.to_string()),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::EnvVar {
                                name: "MOUNT_PATH".to_string(),
                                value: Some(mount_path.to_string()),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::EnvVar {
                                name: "HOST_RW".to_string(),
                                value: Some(host_rw.to_string()),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::EnvVar {
                                name: "NAMESPACE".to_string(),
                                value: Some(namespace.to_string()),
                                ..Default::default()
                            },
                        ]),
                        volume_mounts: Some(vec![VolumeMount {
                            name: volume_name.to_string(),
                            mount_path: mount_path.to_string(),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }],
                    node_selector: Some(BTreeMap::from([(
                        "tembo.io/provisioner".to_string(),
                        "system".to_string(),
                    )])),
                    restart_policy: Some("Never".to_string()),
                    service_account: Some(namespace.to_string()),
                    service_account_name: Some(namespace.to_string()),
                    tolerations: Some(vec![Toleration {
                        key: Some("tembo.io/system".to_string()),
                        operator: Some("Equal".to_string()),
                        value: Some("true".to_string()),
                        ..Default::default()
                    }]),
                    volumes: Some(vec![Volume {
                        name: volume_name.to_string(),
                        ephemeral: Some(EphemeralVolumeSource {
                            volume_claim_template: Some(PersistentVolumeClaimTemplate {
                                metadata: Some(ObjectMeta {
                                    labels: Some(
                                        [
                                            ("app".to_string(), "temback-backup".to_string()),
                                            ("job-name".to_string(), job_name.clone()),
                                            ("coredb.io/name".to_string(), namespace.to_string()),
                                        ]
                                        .into_iter()
                                        .collect(),
                                    ),
                                    ..Default::default()
                                }),
                                spec: PersistentVolumeClaimSpec {
                                    access_modes: Some(vec!["ReadWriteOnce".to_string()]),
                                    storage_class_name: Some("gp3-enc".to_string()),
                                    resources: Some(VolumeResourceRequirements {
                                        requests: Some(
                                            [(
                                                "storage".to_string(),
                                                Quantity(storage_size.to_string()),
                                            )]
                                            .into_iter()
                                            .collect(),
                                        ),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                },
                            }),
                        }),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
            },
            backoff_limit: Some(0),
            ..Default::default()
        }),
        status: None,
    };

    let jobs: Api<Job> = Api::namespaced(kube_client.clone(), namespace);
    jobs.create(&PostParams::default(), &job)
        .await
        .map_err(|e| ErrorInternalServerError(format!("Failed to create backup job: {}", e)))?;
    Ok(())
}

/// Checks the current status of a Kubernetes backup Job without polling.
///
/// This function queries the Kubernetes API for the Job named `temback-backup-{job_id}` in the given namespace.
/// It inspects the Job's status and returns:
/// - `JobStatus::Completed` if the Job succeeded
/// - `JobStatus::Failed` if the Job failed
/// - `JobStatus::Processing` if the Job is still running
/// - `JobStatus::Unknown` if the Job does not exist or status is indeterminate
///
/// # Arguments
/// * `kube_client` - Kubernetes client
/// * `namespace` - Namespace where the Job is running
/// * `job_id` - The backup job identifier (UUID)
///
/// # Returns
/// * `JobStatus` - The current status of the Job
pub async fn get_job_status(kube_client: &KubeClient, namespace: &str, job_id: &str) -> JobStatus {
    let job_name = format!("temback-backup-{job_id}");
    tracing::debug!(namespace = %namespace, job_id = %job_id, job_name = %job_name, "Checking Kubernetes Job status");
    let jobs: Api<K8sJob> = Api::namespaced(kube_client.clone(), namespace);

    let job = match jobs.get(&job_name).await {
        Ok(job) => job,
        Err(e) => {
            tracing::debug!(error = %e, job_name = %job_name, "Job not found or error fetching job");
            return JobStatus::Unknown;
        }
    };

    if let Some(status) = &job.status {
        tracing::debug!(status = ?status, "Fetched job status struct");
        if let Some(succeeded) = status.succeeded {
            tracing::debug!(succeeded = succeeded, "Job succeeded count");
            if succeeded > 0 {
                return JobStatus::Completed;
            }
        }
        if let Some(failed) = status.failed {
            tracing::debug!(failed = failed, "Job failed count");
            if failed > 0 {
                return JobStatus::Failed;
            }
        }
    } else {
        tracing::debug!("Job status is None");
    }
    JobStatus::Processing
}
