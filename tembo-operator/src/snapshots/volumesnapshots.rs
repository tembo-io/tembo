use crate::{
    apis::coredb_types::CoreDB,
    snapshots::{
        volumesnapshotcontents_crd::{
            VolumeSnapshotContent, VolumeSnapshotContentDeletionPolicy,
            VolumeSnapshotContentSource, VolumeSnapshotContentSpec,
            VolumeSnapshotContentVolumeSnapshotRef,
        },
        volumesnapshots_crd::{VolumeSnapshot, VolumeSnapshotSource, VolumeSnapshotSpec},
    },
    Context,
};
use chrono::{DateTime, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{ListParams, Patch, PatchParams},
    client::Client,
    runtime::controller::Action,
    Api, ResourceExt,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

// Main function to reconcile the VolumeSnapshotContent and VolumeSnapshot
pub async fn reconcile_volume_snapshot_restore(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<VolumeSnapshot, Action> {
    let client = ctx.client.clone();

    // Lookup the VolumeSnapshot from the original instance
    let ogvs = lookup_volume_snapshot(cdb, &client).await?;

    // Lookup the VolumeSnapshotContent of the original instance
    let ogvsc = lookup_volume_snapshot_content(cdb, &client, ogvs).await?;

    // Generate the VolumeSnapshotContent and VolumeSnapshot for the restore
    let vsc = generate_volume_snapshot_content(cdb, &ogvsc)?;
    let vs = generate_volume_snapshot(cdb, &vsc)?;

    // Apply the VolumeSnapshotContent and VolumeSnapshot
    apply_volume_snapshot_content(cdb, &client, &vsc).await?;
    apply_volume_snapshot(cdb, &client, &vs).await?;

    // We need to wait for the snapshot to become ready before we can proceed
    is_snapshot_ready(cdb, &client, &vs).await?;

    // Patch the VolumeSnapshotContent with the UID of the VolumeSnapshot
    patch_volume_snapshot_content(cdb, &client, &vsc, &vs).await?;

    Ok(vs)
}

async fn patch_volume_snapshot_content(
    cdb: &CoreDB,
    client: &Client,
    vsc: &VolumeSnapshotContent,
    vs: &VolumeSnapshot,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let vsc_name = vsc.metadata.name.as_ref().ok_or_else(|| {
        error!(
            "VolumeSnapshotContent name is empty for instance: {}.",
            cdb.name_any()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    // Name for the VolumeSnapshot
    let vs_name = vs.metadata.name.as_ref().ok_or_else(|| {
        error!(
            "VolumeSnapshot name is empty for instance: {}.",
            cdb.name_any()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    // Namespace for the VolumeSnapshot
    let vs_ns = vs.metadata.namespace.as_ref().ok_or_else(|| {
        error!(
            "VolumeSnapshot namespace is empty for instance: {}.",
            cdb.name_any()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    // Look up the VolumeSnapshot to get the UID
    let vs_api: Api<VolumeSnapshot> = Api::namespaced(client.clone(), vs_ns);
    let lp = ListParams::default().fields(&format!("metadata.name={}", vs_name));
    let res = vs_api.list(&lp).await.map_err(|e| {
        warn!(
            "Error listing VolumeSnapshots {} in namespace {} for instance {}: {}",
            vsc_name,
            vs_ns,
            cdb.name_any(),
            e,
        );
        Action::requeue(tokio::time::Duration::from_secs(30))
    })?;

    // Check if the VolumeSnapshot (vs) has a UID, requeue if it doesn't
    let vs_uid = match res.items.first().and_then(|vs| vs.metadata.uid.as_ref()) {
        Some(uid) => uid.to_string(),
        None => {
            error!(
                "VolumeSnapshot UID is empty for snapshot {} in namespace {}: {}.",
                vs_name,
                vs_ns,
                cdb.name_any()
            );
            return Err(Action::requeue(tokio::time::Duration::from_secs(10)));
        }
    };

    // Update the VolumeSnapshotContent with the UID of the VolumeSnapshot
    let mut new_vsc = vsc.clone();
    new_vsc.spec.volume_snapshot_ref = VolumeSnapshotContentVolumeSnapshotRef {
        api_version: Some("snapshot.storage.k8s.io/v1".to_string()),
        kind: Some("VolumeSnapshot".to_string()),
        name: vs.metadata.name.clone(),
        namespace: vs.metadata.namespace.clone(),
        uid: Some(vs_uid.to_string()),
        ..VolumeSnapshotContentVolumeSnapshotRef::default()
    };

    // Apply VolumeSnapshotContent (All Namespaces)
    let vsc_api: Api<VolumeSnapshotContent> = Api::all(client.clone());
    debug!("Patching VolumeSnapshotContent for instance: {}", name);
    let ps = PatchParams::apply("cntrlr").force();

    match vsc_api.patch(vsc_name, &ps, &Patch::Apply(&new_vsc)).await {
        Ok(_) => debug!("VolumeSnapshotContent patched successfully for {}.", name),
        Err(e) => {
            error!(
                "Failed to patch VolumeSnapshotContent for instance {}: {}",
                name, e
            );
            return Err(Action::requeue(tokio::time::Duration::from_secs(300)));
        }
    }

    Ok(())
}

async fn is_snapshot_ready(
    cdb: &CoreDB,
    client: &Client,
    vs: &VolumeSnapshot,
) -> Result<(), Action> {
    let name = vs.metadata.name.as_ref().ok_or_else(|| {
        error!(
            "VolumeSnapshot name is empty for instance: {}.",
            cdb.name_any()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let namespace = vs.metadata.namespace.as_ref().ok_or_else(|| {
        error!(
            "VolumeSnapshot namespace is empty for instance: {}.",
            cdb.name_any()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    let vs_api: Api<VolumeSnapshot> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().fields(&format!("metadata.name={}", name));

    // Fetch the VolumeSnapshot to check its status
    let res = vs_api.list(&lp).await.map_err(|e| {
        warn!(
            "Error listing VolumeSnapshots {} for instance {}: {}",
            name,
            cdb.name_any(),
            e,
        );
        Action::requeue(tokio::time::Duration::from_secs(30))
    })?;

    // Check if the first VolumeSnapshot is ready
    if let Some(status) = res.items.first().and_then(|vs| vs.status.as_ref()) {
        if status.ready_to_use.unwrap_or(false) {
            info!(
                "VolumeSnapshot {} is ready for instance {}.",
                name,
                cdb.name_any()
            );
            Ok(())
        } else {
            warn!(
                "VolumeSnapshot {} is not ready yet for instance {}.",
                name,
                cdb.name_any()
            );
            Err(Action::requeue(tokio::time::Duration::from_secs(10)))
        }
    } else {
        error!(
            "VolumeSnapshot {} not found for instance {}",
            name,
            cdb.name_any()
        );
        Err(Action::requeue(tokio::time::Duration::from_secs(30)))
    }
}

async fn apply_volume_snapshot(
    cdb: &CoreDB,
    client: &Client,
    volume_snapshot: &VolumeSnapshot,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let vs_name = volume_snapshot.metadata.name.as_ref().ok_or_else(|| {
        error!(
            "VolumeSnapshot name is empty for instance: {}.",
            cdb.name_any()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    // Namespace for the VolumeSnapshot
    let namespace = volume_snapshot
        .metadata
        .namespace
        .as_deref()
        .ok_or_else(|| {
            error!(
                "VolumeSnapshot namespace is empty for instance: {}.",
                cdb.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

    // Apply VolumeSnapshot (Namespaced)
    let vs_api: Api<VolumeSnapshot> = Api::namespaced(client.clone(), namespace);
    debug!("Patching VolumeSnapshot for instance: {}", name);
    let ps = PatchParams::apply("cntrlr").force();

    match vs_api
        .patch(vs_name, &ps, &Patch::Apply(volume_snapshot))
        .await
    {
        Ok(_) => debug!("VolumeSnapshot created successfully for {}.", name),
        Err(e) => {
            error!(
                "Failed to create VolumeSnapshot for instance {}: {}",
                name, e
            );
            return Err(Action::requeue(tokio::time::Duration::from_secs(300)));
        }
    }

    Ok(())
}

async fn apply_volume_snapshot_content(
    cdb: &CoreDB,
    client: &Client,
    volume_snapshot_content: &VolumeSnapshotContent,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let vsc_name = volume_snapshot_content
        .metadata
        .name
        .as_ref()
        .ok_or_else(|| {
            error!(
                "VolumeSnapshot name is empty for instance: {}.",
                cdb.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

    // Apply VolumeSnapshotContent (All Namespaces)
    let vs_api: Api<VolumeSnapshotContent> = Api::all(client.clone());
    debug!("Patching VolumeSnapshotContent for instance: {}", name);
    let ps = PatchParams::apply("cntrlr").force();

    match vs_api
        .patch(vsc_name, &ps, &Patch::Apply(volume_snapshot_content))
        .await
    {
        Ok(_) => debug!("VolumeSnapshotContent created successfully for {}.", name),
        Err(e) => {
            error!(
                "Failed to create VolumeSnapshotContent for instance {}: {}",
                name, e
            );
            return Err(Action::requeue(tokio::time::Duration::from_secs(300)));
        }
    }

    Ok(())
}

// generate_volume_snapshot_content function generates the VolumeSnapshotContent object
// to map the VolumeSnapshot for the restore
fn generate_volume_snapshot_content(
    cdb: &CoreDB,
    snapshot_content: &VolumeSnapshotContent,
) -> Result<VolumeSnapshotContent, Action> {
    let name = cdb.name_any();
    let namespace = cdb.namespace().ok_or_else(|| {
        error!("CoreDB namespace is empty for instance: {}.", name);
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let snapshot_handle = snapshot_content
        .status
        .as_ref()
        .and_then(|status| status.snapshot_handle.as_ref())
        .ok_or_else(|| {
            error!("Snapshot Handle is empty for instance {}", name);
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?
        .to_string();
    let driver = &snapshot_content.spec.driver;
    let volume_snapshot_class_name = snapshot_content
        .spec
        .volume_snapshot_class_name
        .as_ref()
        .ok_or_else(|| {
            error!("VolumeSnapshotClass name is empty for instance {}", name);
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;
    let snapshot = format!("{}-restore-vs", name);

    let vsc = VolumeSnapshotContent {
        metadata: ObjectMeta {
            name: Some(format!("{}-restore-vsc", name)),
            namespace: Some(namespace.clone()),
            ..ObjectMeta::default()
        },
        spec: VolumeSnapshotContentSpec {
            deletion_policy: VolumeSnapshotContentDeletionPolicy::Retain,
            driver: driver.to_string(),
            source: VolumeSnapshotContentSource {
                snapshot_handle: Some(snapshot_handle),
                ..VolumeSnapshotContentSource::default()
            },
            volume_snapshot_class_name: Some(volume_snapshot_class_name.to_string()),
            volume_snapshot_ref: VolumeSnapshotContentVolumeSnapshotRef {
                api_version: Some("snapshot.storage.k8s.io/v1".to_string()),
                kind: Some("VolumeSnapshot".to_string()),
                name: Some(snapshot),
                namespace: Some(namespace.clone()),
                ..VolumeSnapshotContentVolumeSnapshotRef::default()
            },
            ..VolumeSnapshotContentSpec::default()
        },
        status: None,
    };

    Ok(vsc)
}

// generate_volume_snapshot function generates the VolumeSnapshot object and ties
// it back to the VolumeSnapshotContent
fn generate_volume_snapshot(
    cdb: &CoreDB,
    snapshot_content: &VolumeSnapshotContent,
) -> Result<VolumeSnapshot, Action> {
    let name = cdb.name_any();
    let namespace = cdb.namespace().ok_or_else(|| {
        error!("CoreDB namespace is empty for instance {}", name);
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let volume_snapshot_content_name =
        snapshot_content.metadata.name.as_ref().ok_or_else(|| {
            error!("VolumeSnapshotContent name is empty for instance {}", name);
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;
    let volume_snapshot_class_name = snapshot_content
        .spec
        .volume_snapshot_class_name
        .as_ref()
        .ok_or_else(|| {
            error!("VolumeSnapshotClass name is empty for instance {}", name);
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

    let vs = VolumeSnapshot {
        metadata: ObjectMeta {
            name: Some(format!("{}-restore-vs", name)),
            namespace: Some(namespace),
            ..ObjectMeta::default()
        },
        spec: VolumeSnapshotSpec {
            source: VolumeSnapshotSource {
                volume_snapshot_content_name: Some(volume_snapshot_content_name.to_string()),
                ..VolumeSnapshotSource::default()
            },
            volume_snapshot_class_name: Some(volume_snapshot_class_name.to_string()),
        },
        status: None,
    };
    Ok(vs)
}

// lookup_volume_snapshot function looks up the VolumeSnapshot object from the
// original instance you are restoring from
async fn lookup_volume_snapshot(cdb: &CoreDB, client: &Client) -> Result<VolumeSnapshot, Action> {
    // name will be the name of the original instance
    let og_instance_name = cdb
        .spec
        .restore
        .as_ref()
        .map(|r| r.server_name.clone())
        .ok_or_else(|| {
            error!(
                "CoreDB restore server_name is empty for instance {}",
                cdb.name_any()
            );
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

    debug!(
        "Looking up VolumeSnapshots for original instance: {}",
        og_instance_name
    );

    // todo: This is a temporary fix to get the VolumeSnapshot from the same namespace as the
    // instance you are attempting to restore from.  We need to figure out a better way of
    // doing this in case someone wants to name a namespace differently than the instance name.
    // At Tembo we assume that the namespace and the name of the instance name are the same.
    let volume_snapshot_api: Api<VolumeSnapshot> =
        Api::namespaced(client.clone(), &og_instance_name);

    let label_selector = format!("cnpg.io/cluster={}", og_instance_name);
    let lp = ListParams::default().labels(&label_selector);
    let result = volume_snapshot_api.list(&lp).await.map_err(|e| {
        error!(
            "Error listing VolumeSnapshots for instance {}: {}",
            og_instance_name, e
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    debug!(
        "Found {} VolumeSnapshots for instance {}",
        result.items.len(),
        og_instance_name
    );

    // Set recovery_target_time if it's set in the CoreDB spec as DateTime<Utc>
    let recovery_target_time: Option<DateTime<Utc>> = cdb
        .spec
        .restore
        .as_ref()
        .and_then(|r| r.recovery_target_time.as_deref())
        .and_then(|time_str| DateTime::parse_from_rfc3339(time_str).ok())
        .map(|dt| dt.with_timezone(&Utc));

    debug!("Recovery target time: {:?}", recovery_target_time);

    // Filter snapshots that are ready to use and sort them by creation timestamp in descending order
    let snapshots: Vec<VolumeSnapshot> = result
        .into_iter()
        .filter(|vs| {
            vs.status
                .as_ref()
                .and_then(|s| s.ready_to_use)
                .unwrap_or(false)
        })
        .filter(|vs| {
            vs.metadata
                .annotations
                .as_ref()
                .and_then(|ann| ann.get("cnpg.io/instanceRole"))
                .map(|role| role == "primary")
                .unwrap_or(false)
        })
        .collect();

    debug!(
        "Filtered {} VolumeSnapshots for primary role and readiness",
        snapshots.len()
    );

    match find_closest_snapshot(snapshots, recovery_target_time) {
        Some(snapshot) => {
            debug!(
                "Selected VolumeSnapshot: {}",
                snapshot
                    .metadata
                    .name
                    .as_deref()
                    .map_or("unknown", |name| name)
            );
            Ok(snapshot)
        }
        None => {
            error!(
                "No suitable VolumeSnapshot found for instance {}",
                og_instance_name
            );
            Err(Action::requeue(tokio::time::Duration::from_secs(300)))
        }
    }
}

fn find_closest_snapshot(
    snapshots: Vec<VolumeSnapshot>,
    recovery_target_time: Option<DateTime<Utc>>,
) -> Option<VolumeSnapshot> {
    let filtered_snapshots: Vec<_> = snapshots
        .into_iter()
        .filter_map(|snapshot| {
            let end_time_str = snapshot
                .metadata
                .annotations
                .as_ref()?
                .get("cnpg.io/snapshotEndTime")?;
            let end_time = DateTime::parse_from_rfc3339(end_time_str)
                .ok()?
                .with_timezone(&Utc);
            Some((snapshot, end_time))
        })
        .collect();

    debug!(
        "Filtered snapshots based on end time: {:?}",
        filtered_snapshots
    );

    match recovery_target_time {
        Some(target_time) => {
            debug!("Recovery target time is specified: {:?}", target_time);
            let closest_snapshot = filtered_snapshots
                .into_iter()
                .filter(|(_, end_time)| *end_time <= target_time)
                .min_by_key(|(_, end_time)| (target_time - *end_time).num_seconds().abs())
                .map(|(snapshot, _)| snapshot);

            debug!("Selected closest snapshot: {:?}", closest_snapshot);
            closest_snapshot
        }
        None => {
            debug!("No recovery target time specified, selecting the latest snapshot.");
            let latest_snapshot = filtered_snapshots
                .into_iter()
                .max_by_key(|(_, end_time)| *end_time)
                .map(|(snapshot, _)| snapshot);

            debug!("Selected latest snapshot: {:?}", latest_snapshot);
            latest_snapshot
        }
    }
}

async fn lookup_volume_snapshot_content(
    cdb: &CoreDB,
    client: &Client,
    snapshot: VolumeSnapshot,
) -> Result<VolumeSnapshotContent, Action> {
    // The name of the VolumeSnapshotContext is in the status.boundVolumeSnapshotContentName field
    // in the VolumeSnapshot
    let name = snapshot
        .status
        .as_ref()
        .and_then(|s| s.bound_volume_snapshot_content_name.clone())
        .ok_or_else(|| {
            error!("Snapshot status is empty for instance {}", cdb.name_any());
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?;

    // Lookup the VolumeSnapshotContent object, since it's not namespaced we will
    // need to filter on all objects in the cluster
    let volume_snapshot_content_api: Api<VolumeSnapshotContent> = Api::all(client.clone());
    match volume_snapshot_content_api.get(&name).await {
        Ok(vsc) => Ok(vsc),
        Err(e) => {
            error!(
                "Failed to get VolumeSnapshotContent for instance {}: {}",
                cdb.name_any(),
                e
            );
            Err(Action::requeue(tokio::time::Duration::from_secs(300)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apis::coredb_types::CoreDB,
        snapshots::volumesnapshotcontents_crd::{
            VolumeSnapshotContent, VolumeSnapshotContentSource, VolumeSnapshotContentSpec,
            VolumeSnapshotContentStatus,
        },
    };
    use chrono::DateTime;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    #[test]
    fn test_generate_volume_snapshot_content() {
        let cdb_yaml = r#"
        apiVersion: coredb.io/v1alpha1
        kind: CoreDB
        metadata:
          name: test
          namespace: default
        spec:
          backup:
            destinationPath: s3://tembo-backup/sample-standard-backup
            encryption: ""
            retentionPolicy: "30"
            schedule: 17 9 * * *
            endpointURL: http://minio:9000
            volumeSnapshot:
              enabled: true
              snapshotClass: "csi-vsc"
          image: quay.io/tembo/tembo-pg-cnpg:15.3.0-5-48d489e
          port: 5432
          replicas: 1
          resources:
            limits:
              cpu: "1"
              memory: 0.5Gi
          serviceAccountTemplate:
            metadata:
              annotations:
                eks.amazonaws.com/role-arn: arn:aws:iam::012345678901:role/aws-iam-role-iam
          sharedirStorage: 1Gi
          stop: false
          storage: 1Gi
          storageClass: "gp3-enc"
          uid: 999
        "#;
        let cdb: CoreDB = serde_yaml::from_str(cdb_yaml).expect("Failed to parse YAML");

        let snapshot_content = VolumeSnapshotContent {
            metadata: ObjectMeta {
                name: Some("test-snapshot-content".to_string()),
                namespace: cdb.namespace(),
                ..ObjectMeta::default()
            },
            spec: VolumeSnapshotContentSpec {
                source: VolumeSnapshotContentSource {
                    volume_handle: Some("test-volume-handle".to_string()),
                    ..VolumeSnapshotContentSource::default()
                },
                driver: "test-driver".to_string(),
                volume_snapshot_class_name: Some("test-class".to_string()),
                ..VolumeSnapshotContentSpec::default()
            },
            status: Some(VolumeSnapshotContentStatus {
                creation_time: Some(1708542600948000000),
                ready_to_use: Some(true),
                restore_size: Some(10737418240),
                snapshot_handle: Some("snap-01234567abcdef890".to_string()),
                ..VolumeSnapshotContentStatus::default()
            }),
        };

        let result = generate_volume_snapshot_content(&cdb, &snapshot_content).unwrap();

        assert_eq!(result.spec.driver, "test-driver");
        assert_eq!(
            result.spec.source.snapshot_handle,
            Some("snap-01234567abcdef890".to_string())
        );
        assert_eq!(
            result.spec.volume_snapshot_class_name,
            Some("test-class".to_string())
        );
    }

    #[test]
    fn test_generate_volume_snapshot() {
        let cdb_yaml = r#"
        apiVersion: coredb.io/v1alpha1
        kind: CoreDB
        metadata:
          name: test
          namespace: default
        spec:
          backup:
            destinationPath: s3://tembo-backup/sample-standard-backup
            encryption: ""
            retentionPolicy: "30"
            schedule: 17 9 * * *
            endpointURL: http://minio:9000
            volumeSnapshot:
              enabled: true
              snapshotClass: "csi-vsc"
          image: quay.io/tembo/tembo-pg-cnpg:15.3.0-5-48d489e
          port: 5432
          replicas: 1
          resources:
            limits:
              cpu: "1"
              memory: 0.5Gi
          serviceAccountTemplate:
            metadata:
              annotations:
                eks.amazonaws.com/role-arn: arn:aws:iam::012345678901:role/aws-iam-role-iam
          sharedirStorage: 1Gi
          stop: false
          storage: 1Gi
          storageClass: "gp3-enc"
          uid: 999
        "#;
        let cdb: CoreDB = serde_yaml::from_str(cdb_yaml).expect("Failed to parse YAML");

        let snapshot_content = VolumeSnapshotContent {
            metadata: ObjectMeta {
                name: Some("test-snapshot-content".to_string()),
                namespace: Some("default".to_string()), // Ensure namespace matches CoreDB for the test's purpose
                ..ObjectMeta::default()
            },
            spec: VolumeSnapshotContentSpec {
                source: VolumeSnapshotContentSource {
                    volume_handle: Some("test-volume-handle".to_string()), // This might not be relevant for this test
                    ..VolumeSnapshotContentSource::default()
                },
                driver: "test-driver".to_string(), // Not directly relevant for this test
                volume_snapshot_class_name: Some("test-class".to_string()),
                ..VolumeSnapshotContentSpec::default()
            },
            status: None,
        };

        // Execute the function under test
        let result = generate_volume_snapshot(&cdb, &snapshot_content).unwrap();

        // Assertions
        assert_eq!(
            result.metadata.name.unwrap(),
            format!("{}-restore-vs", cdb.name_any())
        );
        assert_eq!(
            result.spec.source.volume_snapshot_content_name,
            Some("test-snapshot-content".to_string())
        );
        assert_eq!(
            result.spec.volume_snapshot_class_name,
            Some("test-class".to_string())
        );
        // The namespace of the generated VolumeSnapshot should match the namespace of the CoreDB
        assert_eq!(result.metadata.namespace.unwrap(), "default");
    }

    fn create_volume_snapshot(name: &str, snapshot_end_time: &str) -> VolumeSnapshot {
        let mut annotations = BTreeMap::new();
        annotations.insert(
            "cnpg.io/snapshotEndTime".to_string(),
            snapshot_end_time.to_string(),
        );

        VolumeSnapshot {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                annotations: Some(annotations),
                ..ObjectMeta::default()
            },
            spec: VolumeSnapshotSpec {
                ..VolumeSnapshotSpec::default()
            },
            status: None,
        }
    }

    #[test]
    fn test_find_closest_snapshot_pitr() {
        let recovery_target_time_str = "2024-03-06T00:00:00Z";
        let recovery_target_time = DateTime::parse_from_rfc3339(recovery_target_time_str)
            .unwrap()
            .with_timezone(&Utc);

        let snapshots = vec![
            create_volume_snapshot("snapshot1", "2024-03-05T23:50:00Z"),
            create_volume_snapshot("snapshot2", "2024-03-05T22:00:00Z"),
            create_volume_snapshot("snapshot3", "2024-03-05T23:55:00Z"),
            create_volume_snapshot("snapshot4", "2024-03-05T21:00:00Z"),
            create_volume_snapshot("snapshot5", "2024-03-06T00:01:00Z"),
        ];

        let closest_snapshot =
            find_closest_snapshot(snapshots, Some(recovery_target_time)).unwrap();
        assert_eq!(closest_snapshot.metadata.name.unwrap(), "snapshot3");
    }

    #[test]
    fn test_find_latest_snapshot_when_target_time_empty() {
        let snapshots = vec![
            create_volume_snapshot("snapshot1", "2024-03-05T20:00:00Z"),
            create_volume_snapshot("snapshot2", "2024-03-05T22:00:00Z"),
            create_volume_snapshot("snapshot3", "2024-03-05T23:00:00Z"),
            create_volume_snapshot("snapshot4", "2024-03-05T21:00:00Z"),
            create_volume_snapshot("snapshot5", "2024-03-06T00:01:00Z"),
        ];

        let latest_snapshot = find_closest_snapshot(snapshots, None).unwrap();
        assert_eq!(latest_snapshot.metadata.name.unwrap(), "snapshot5");
    }
}
