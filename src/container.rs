use k8s_openapi::api::core::v1::{Capabilities, Container, SecurityContext, VolumeMount};

use crate::config::Config;

// Create a Container object that will be injected into the Pod
pub fn create_init_container(config: &Config) -> Container {
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
        name: "tembo-bootstrap".to_string(),
        image: Some(config.container_image.to_string()),
        image_pull_policy: Some("IfNotPresent".to_string()),
        command: Some(vec![
            "/bin/bash".to_string(),
            "-c".to_string(),
            "if [ -d /var/lib/postgresql/data/tembo ]; then if [ -z \"$(ls -A /var/lib/postgresql/data/tembo)\" ]; then cp -Rp /tmp/pg_sharedir/. /var/lib/postgresql/data/tembo; fi; else mkdir -p /var/lib/postgresql/data/tembo && cp -Rp /tmp/pg_sharedir/. /var/lib/postgresql/data/tembo; fi".to_string(),
        ]),
        security_context: Some(security_context),
        volume_mounts: Some(volume_mounts),
        termination_message_path: Some("/dev/termination-log".to_string()),
        termination_message_policy: Some("File".to_string()),
        ..Default::default()
    }
}
