use controller::cloudnativepg::clusters::Cluster;
use k8s_openapi::api::core::v1::{Capabilities, Container, SecurityContext, VolumeMount};
use kube::{Api, Client};
use tracing::*;

use crate::config::Config;

// Create a Container object that will be injected into the Pod
#[instrument(skip(client))]
pub async fn create_init_container(
    config: &Config,
    client: &Client,
    namespace: &str,
    cluster_name: &str,
) -> Container {
    // Get the correct container image to use from Cluster
    let cluster_api: Api<Cluster> = Api::namespaced(client.clone(), namespace);
    let cluster = cluster_api.get(cluster_name).await.unwrap();

    // Extract the image we need to use
    let image = cluster
        .spec
        .image_name
        .unwrap_or_else(|| config.container_image.clone());

    // Add in mounted volumes
    let volume_mounts = vec![
        VolumeMount {
            name: "pgdata".to_string(),
            mount_path: "/var/lib/postgresql/data".to_string(),
            ..Default::default()
        },
        VolumeMount {
            name: "scratch-data".to_string(),
            mount_path: "/run".to_string(),
            ..Default::default()
        },
        VolumeMount {
            name: "scratch-data".to_string(),
            mount_path: "/controller".to_string(),
            ..Default::default()
        },
    ];

    // Create the SecurityContext for the initContainer
    let security_context = SecurityContext {
        allow_privilege_escalation: Some(false),
        capabilities: Some(Capabilities {
            drop: Some(vec!["ALL".to_string()]),
            ..Default::default()
        }),
        privileged: Some(false),
        read_only_root_filesystem: Some(true),
        run_as_non_root: Some(true),
        ..Default::default()
    };

    // Create the initContainer
    Container {
        name: config.init_container_name.to_string(),
        image: Some(image),
        image_pull_policy: Some("IfNotPresent".to_string()),
        command: Some(vec![
            "/bin/bash".to_string(),
            "-c".to_string(),
            "mkdir -p /var/lib/postgresql/data/tembo; if [ -d /tmp/pg_sharedir ] && [ -z \"$(ls -A /var/lib/postgresql/data/tembo)\" ]; then cp -Rp /tmp/pg_sharedir/. /var/lib/postgresql/data/tembo; fi".to_string(),
            "if [-d /var/lib/postgresql/data/lost+found ]; then rmdir /var/lib/postgresql/data/lost+found; fi".to_string(),
        ]),
        security_context: Some(security_context),
        volume_mounts: Some(volume_mounts),
        termination_message_path: Some("/dev/termination-log".to_string()),
        termination_message_policy: Some("File".to_string()),
        ..Default::default()
    }
}

pub fn add_volume_mounts(container: &mut Container, volume_mount: VolumeMount) {
    // Check to make sure we only add the volume once
    if container
        .volume_mounts
        .as_ref()
        .is_some_and(|volume_mounts| {
            volume_mounts
                .iter()
                .any(|v| v.name == volume_mount.name && v.mount_path == volume_mount.mount_path)
        })
    {
        debug!(
            "Container already has volume mount {:?}, skipping",
            volume_mount
        );
    } else {
        // Push the new volume mount to the list of volume mounts in the container
        container
            .volume_mounts
            .get_or_insert_with(Vec::new)
            .push(volume_mount);
    }
}
