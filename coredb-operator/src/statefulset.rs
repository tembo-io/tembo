use crate::{
    apis::coredb_types::CoreDB,
    defaults::{default_image, default_postgres_exporter_image},
    Context, Error, Result,
};
use k8s_openapi::{
    api::{
        apps::v1::{StatefulSet, StatefulSetSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, EnvVarSource, ExecAction, PersistentVolumeClaim,
            PersistentVolumeClaimSpec, Pod, PodSpec, PodTemplateSpec, Probe, ResourceRequirements,
            SecretKeySelector, SecurityContext, VolumeMount,
        },
    },
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector},
};
use kube::{
    api::{Api, DeleteParams, ListParams, ObjectMeta, Patch, PatchParams, ResourceExt},
    Resource,
};
use std::{str, thread, time::Duration};

use k8s_openapi::{
    api::core::v1::{ConfigMapVolumeSource, EmptyDirVolumeSource, HTTPGetAction, Volume},
    apimachinery::pkg::util::intstr::IntOrString,
};
use std::{collections::BTreeMap, sync::Arc};
use tracing::{debug, error, info, warn};

use crate::postgres_exporter::{EXPORTER_CONFIGMAP, EXPORTER_VOLUME, QUERIES_YAML};
const PKGLIBDIR: &str = "/usr/lib/postgresql/15/lib";
const SHAREDIR: &str = "/usr/share/postgresql/15";
const DATADIR: &str = "/var/lib/postgresql/data";
const PROM_CFG_DIR: &str = "/prometheus";

pub fn stateful_set_from_cdb(cdb: &CoreDB) -> StatefulSet {
    let ns = cdb.namespace().unwrap();
    let name = cdb.name_any();
    let oref = cdb.controller_owner_ref(&()).unwrap();
    let mut pvc_requests_datadir: BTreeMap<String, Quantity> = BTreeMap::new();
    pvc_requests_datadir.insert("storage".to_string(), cdb.spec.storage.clone());
    let mut pvc_requests_sharedir: BTreeMap<String, Quantity> = BTreeMap::new();
    pvc_requests_sharedir.insert("storage".to_string(), cdb.spec.sharedirStorage.clone());
    let mut pvc_requests_pkglibdir: BTreeMap<String, Quantity> = BTreeMap::new();
    pvc_requests_pkglibdir.insert("storage".to_string(), cdb.spec.pkglibdirStorage.clone());
    let backup = &cdb.spec.backup;
    let image = &cdb.spec.image;

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());
    labels.insert("statefulset".to_owned(), name.to_owned());

    let postgres_env = Some(vec![
        EnvVar {
            name: "POSTGRES_PASSWORD".to_owned(),
            value: None,
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    key: "password".to_string(),
                    name: Some(format!("{}-connection", &name)),
                    optional: None,
                }),
                ..EnvVarSource::default()
            }),
        },
        EnvVar {
            name: "WALG_S3_PREFIX".to_owned(),
            value: backup.destinationPath.clone(),
            value_from: None,
        },
        EnvVar {
            name: "WALG_S3_SSE".to_owned(),
            value: backup.encryption.clone(),
            value_from: None,
        },
    ]);

    let postgres_volume_mounts = Some(vec![
        VolumeMount {
            name: "data".to_owned(),
            mount_path: DATADIR.to_owned(),
            ..VolumeMount::default()
        },
        VolumeMount {
            name: "certs".to_owned(),
            mount_path: "/certs".to_owned(),
            ..VolumeMount::default()
        },
        VolumeMount {
            name: "pkglibdir".to_owned(),
            mount_path: PKGLIBDIR.to_owned(),
            ..VolumeMount::default()
        },
        VolumeMount {
            name: "sharedir".to_owned(),
            mount_path: SHAREDIR.to_owned(),
            ..VolumeMount::default()
        },
    ]);
    let mut containers = vec![
        // This container for running postgresql
        Container {
            args: Some(vec![
                "-c".to_string(),
                "ssl=on".to_string(),
                "-c".to_string(),
                "ssl_cert_file=/certs/server.crt".to_string(),
                "-c".to_string(),
                "ssl_key_file=/certs/server.key".to_string(),
            ]),
            env: postgres_env.clone(),
            security_context: Some(SecurityContext {
                run_as_user: Some(cdb.spec.uid as i64),
                allow_privilege_escalation: Some(false),
                ..SecurityContext::default()
            }),
            name: "postgres".to_string(),
            image: if image.is_empty() {
                Some(default_image())
            } else {
                Some(image.clone())
            },
            resources: Some(cdb.spec.resources.clone()),
            ports: Some(vec![ContainerPort {
                container_port: 5432,
                ..ContainerPort::default()
            }]),
            volume_mounts: postgres_volume_mounts.clone(),
            readiness_probe: Some(Probe {
                exec: Some(ExecAction {
                    command: Some(vec![String::from("pg_isready")]),
                }),
                initial_delay_seconds: Some(3),
                ..Probe::default()
            }),
            ..Container::default()
        },
    ];


    if cdb.spec.postgresExporterEnabled {
        containers.push(Container {
            name: "postgres-exporter".to_string(),
            image: Some(default_postgres_exporter_image()),
            args: Some(vec![
                "--auto-discover-databases".to_string(),
                // "--log.level=debug".to_string(),
            ]),
            env: Some(vec![
                EnvVar {
                    name: "DATA_SOURCE_NAME".to_string(),
                    value: Some("postgresql://postgres_exporter@localhost:5432/postgres".to_string()),
                    ..EnvVar::default()
                },
                EnvVar {
                    name: "PG_EXPORTER_EXTEND_QUERY_PATH".to_string(),
                    value: Some(format!("{PROM_CFG_DIR}/{QUERIES_YAML}")),
                    ..EnvVar::default()
                },
            ]),
            security_context: Some(SecurityContext {
                run_as_user: Some(65534),
                allow_privilege_escalation: Some(false),
                ..SecurityContext::default()
            }),
            ports: Some(vec![ContainerPort {
                container_port: 9187,
                name: Some("metrics".to_string()),
                protocol: Some("TCP".to_string()),
                ..ContainerPort::default()
            }]),
            readiness_probe: Some(Probe {
                http_get: Some(HTTPGetAction {
                    path: Some("/metrics".to_string()),
                    port: IntOrString::String("metrics".to_string()),
                    ..HTTPGetAction::default()
                }),
                initial_delay_seconds: Some(3),
                ..Probe::default()
            }),
            volume_mounts: Some(vec![VolumeMount {
                name: EXPORTER_VOLUME.to_owned(),
                mount_path: PROM_CFG_DIR.to_string(),
                ..VolumeMount::default()
            }]),
            ..Container::default()
        });
    }

    // 0 replicas on sts when stopping
    // 1 replica in all other cases
    let replicas = match cdb.spec.stop {
        true => 0,
        false => 1,
    };

    let sts: StatefulSet = StatefulSet {
        metadata: ObjectMeta {
            name: Some(name),
            namespace: Some(ns),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref]),
            ..ObjectMeta::default()
        },
        spec: Some(StatefulSetSpec {
            replicas: Some(replicas),
            selector: LabelSelector {
                match_expressions: None,
                match_labels: Some(labels.clone()),
            },
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    service_account_name: Some(format!("{}-sa", cdb.name_any())),
                    containers,
                    init_containers: Option::from(vec![Container {
                        env: postgres_env,
                        name: "pg-directory-init".to_string(),
                        image: if image.is_empty() {
                            Some(default_image())
                        } else {
                            Some(image.clone())
                        },
                        volume_mounts: postgres_volume_mounts,
                        security_context: Some(SecurityContext {
                            // Run the init container as root
                            run_as_user: Some(0),
                            allow_privilege_escalation: Some(false),
                            ..SecurityContext::default()
                        }),
                        // When we have our own PG container,
                        // this will be refactored: this is assuming the
                        // content of the docker entrypoint script
                        // https://github.com/docker-library/postgres/blob/master/docker-entrypoint.sh
                        args: Some(vec![
                            "/bin/bash".to_string(),
                            "-c".to_string(),
                            "\
                            set -e
                            source /usr/local/bin/docker-entrypoint.sh
                            set -x

                            # ext4 will create this directory
                            # on AWS block storage.
                            rmdir $PGDATA/lost+found || true

                            docker_setup_env
                            docker_create_db_directories
                            
                            # remove stale files
                            find $(pg_config --sharedir) -user root -type f -ctime +1 -delete
                            find $(pg_config --pkglibdir) -user root -type f -ctime +1 -delete
                            # copy system files from image cache to volumes directories
                            cp -r /tmp/pg_sharedir/* $(pg_config --sharedir)/
                            cp -r /tmp/pg_pkglibdir/* $(pg_config --pkglibdir)/
                            
                            # set permissions to the places that trunk writes
                            # sharedir
                            chown postgres:postgres $(pg_config --sharedir)
                            chmod 2775 $(pg_config --sharedir)
                            # sharedir/extension
                            chown postgres:postgres $(pg_config --sharedir)/extension 
                            chmod 2775 $(pg_config --sharedir)/extension
                            # sharedir/bitcode
                            chown postgres:postgres $(pg_config --pkglibdir)/bitcode || :
                            chmod 2775 $(pg_config --pkglibdir)/bitcode || :
                            # pkglibdir
                            chown postgres:postgres $(pg_config --pkglibdir)
                            chmod 2775 $(pg_config --pkglibdir)

                            # https://www.postgresql.org/docs/current/ssl-tcp.html
                            cd /certs
                            openssl req -new -x509 -days 365 -nodes -text -out server.crt \
                              -keyout server.key -subj '/CN=selfsigned.coredb.io'
                            chmod og-rwx server.key
                            chown -R postgres:postgres /certs
                        "
                            .to_string(),
                        ]),
                        ..Container::default()
                    }]),
                    volumes: Some(vec![
                        Volume {
                            name: "certs".to_owned(),
                            empty_dir: Some(EmptyDirVolumeSource {
                                ..EmptyDirVolumeSource::default()
                            }),
                            ..Volume::default()
                        },
                        Volume {
                            config_map: Some(ConfigMapVolumeSource {
                                name: Some(EXPORTER_VOLUME.to_owned()),
                                ..ConfigMapVolumeSource::default()
                            }),
                            name: EXPORTER_CONFIGMAP.to_owned(),
                            ..Volume::default()
                        },
                    ]),
                    ..PodSpec::default()
                }),
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..ObjectMeta::default()
                }),
            },
            volume_claim_templates: Some(vec![
                PersistentVolumeClaim {
                    metadata: ObjectMeta {
                        name: Some("data".to_string()),
                        ..ObjectMeta::default()
                    },
                    spec: Some(PersistentVolumeClaimSpec {
                        access_modes: Some(vec!["ReadWriteOnce".to_owned()]),
                        resources: Some(ResourceRequirements {
                            limits: None,
                            requests: Some(pvc_requests_datadir),
                        }),
                        ..PersistentVolumeClaimSpec::default()
                    }),
                    status: None,
                },
                PersistentVolumeClaim {
                    metadata: ObjectMeta {
                        name: Some("sharedir".to_string()),
                        ..ObjectMeta::default()
                    },
                    spec: Some(PersistentVolumeClaimSpec {
                        access_modes: Some(vec!["ReadWriteOnce".to_owned()]),
                        resources: Some(ResourceRequirements {
                            limits: None,
                            requests: Some(pvc_requests_sharedir),
                        }),
                        ..PersistentVolumeClaimSpec::default()
                    }),
                    status: None,
                },
                PersistentVolumeClaim {
                    metadata: ObjectMeta {
                        name: Some("pkglibdir".to_string()),
                        ..ObjectMeta::default()
                    },
                    spec: Some(PersistentVolumeClaimSpec {
                        access_modes: Some(vec!["ReadWriteOnce".to_owned()]),
                        resources: Some(ResourceRequirements {
                            limits: None,
                            requests: Some(pvc_requests_pkglibdir),
                        }),
                        ..PersistentVolumeClaimSpec::default()
                    }),
                    status: None,
                },
            ]),
            ..StatefulSetSpec::default()
        }),
        ..StatefulSet::default()
    };
    sts
}

fn diff_pvcs(expected: &[String], actual: &[String]) -> Vec<String> {
    let mut to_create = vec![];
    for pvc in expected {
        if !actual.contains(pvc) {
            to_create.push(pvc.to_string());
        }
    }
    to_create
}

async fn list_pvcs(ctx: Arc<Context>, sts_name: &str, sts_namespace: &str) -> Result<Vec<String>, Error> {
    let label_selector = format!("statefulset={sts_name}");
    let list_params = ListParams::default().labels(&label_selector);
    let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(ctx.client.clone(), sts_namespace);

    // list all PVCs in namespace
    let all_pvcs = pvc_api.list(&list_params).await?;
    Ok(all_pvcs
        .into_iter()
        .map(|pvc| pvc.metadata.name.unwrap())
        .collect())
}

pub async fn handle_create_update(
    cdb: &CoreDB,
    pvcs_to_update: Vec<(String, Quantity)>,
    sts_api: Api<StatefulSet>,
    pvcs_to_create: Vec<String>,
    ctx: Arc<Context>,
    sts_namespace: &str,
    sts_name: &str,
) -> Result<(), Error> {
    let client = ctx.client.clone();
    if !pvcs_to_update.is_empty() {
        delete_sts_no_cascade(&sts_api, sts_name).await?;
        let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(ctx.client.clone(), sts_namespace);
        for (pvc_full_name, qty) in pvcs_to_update {
            update_pvc(&pvc_api, &pvc_full_name, qty).await?;
        }
    }

    if !pvcs_to_create.is_empty() {
        delete_sts_no_cascade(&sts_api, sts_name).await?;
        let primary_pod = cdb.primary_pod(client.clone()).await?;
        let pod_api: Api<Pod> = Api::namespaced(client.clone(), sts_namespace);
        let prim_pod_name = primary_pod
            .metadata
            .name
            .ok_or(Error::PodError("pod missing".to_owned()))?;
        warn!("deleting pod to attach pvc: {}", prim_pod_name);
        pod_api.delete(&prim_pod_name, &DeleteParams::default()).await?;
    }
    Ok(())
}

pub async fn reconcile_sts(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();

    let sts: StatefulSet = stateful_set_from_cdb(cdb);

    let sts_api: Api<StatefulSet> = Api::namespaced(client.clone(), &sts.clone().metadata.namespace.unwrap());

    let sts_name = sts.clone().metadata.name.unwrap();
    let sts_namespace = sts.clone().metadata.namespace.unwrap();

    // If cdb is running and storage is resized in CoreDB Custom Resource then follow steps to resize PVC
    // Reference Article: https://itnext.io/resizing-statefulset-persistent-volumes-with-zero-downtime-916ebc65b1d4

    // determine pvcs that changed or need to be created
    let (pvcs_to_update, pvcs_to_create) = match cdb.status.is_some() && cdb.status.clone().unwrap().running {
        true => {
            let mut pvcs_to_update = Vec::new();
            let data_pvc_name = pvc_full_name("data", &sts_name);
            let pkglib_pvc_name = pvc_full_name("pkglibdir", &sts_name);
            let share_pvc_name = pvc_full_name("sharedir", &sts_name);

            // reconcile expected vs actual pvcs, this is what needs to be created
            let expected_pvcs = vec![
                data_pvc_name.clone(),
                pkglib_pvc_name.clone(),
                share_pvc_name.clone(),
            ];
            let actual_pvcs = list_pvcs(ctx.clone(), &sts_name, &sts_namespace).await?;
            // if there is a diff, it needs to be created. assumes we never delete a pvc.
            let pvcs_to_create = diff_pvcs(&expected_pvcs, &actual_pvcs);
            debug!("pvcs_to_create: {:?}", pvcs_to_create);

            // determine if PVCs changed
            if cdb.status.clone().unwrap().storage != cdb.spec.storage {
                pvcs_to_update.push((data_pvc_name, cdb.spec.storage.clone()));
            }
            if cdb.status.clone().unwrap().sharedirStorage != cdb.spec.sharedirStorage {
                pvcs_to_update.push((share_pvc_name, cdb.spec.sharedirStorage.clone()));
            }
            if cdb.status.clone().unwrap().pkglibdirStorage != cdb.spec.pkglibdirStorage {
                pvcs_to_update.push((pkglib_pvc_name, cdb.spec.pkglibdirStorage.clone()));
            }
            debug!("pvcs_to_update: {:?}", pvcs_to_update);
            (pvcs_to_update, pvcs_to_create)
        }
        false => {
            debug!("cdb is not running");
            (vec![], vec![])
        }
    };

    let create_update_result = handle_create_update(
        cdb,
        pvcs_to_update,
        sts_api.clone(),
        pvcs_to_create,
        ctx,
        &sts_namespace,
        &sts_name,
    )
    .await;
    // never panic when handling create/update
    // this operation includes deleting a STS, and pods
    // ensure we always make it to the PATCH operation below
    match create_update_result {
        Ok(_) => {
            debug!("successfully create_update sts/pvc resources");
        }
        Err(e) => {
            error!("create_update_result: {:?}", e);
        }
    }

    let ps = PatchParams::apply("cntrlr").force();
    let _o = sts_api
        .patch(&sts.clone().metadata.name.unwrap(), &ps, &Patch::Apply(&sts))
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}

async fn delete_sts_no_cascade(sts_api: &Api<StatefulSet>, sts_name: &str) -> Result<(), Error> {
    let delete_params: DeleteParams = DeleteParams {
        dry_run: false,
        grace_period_seconds: None,
        propagation_policy: Some(kube::api::PropagationPolicy::Orphan),
        preconditions: None,
    };
    info!("deleting_sts_no_cascade: {}", sts_name);
    let _ = sts_api
        .delete(sts_name, &delete_params)
        .await
        .map_err(Error::KubeError)?;
    thread::sleep(Duration::from_millis(3000));
    Ok(())
}

fn pvc_full_name(pvc_name: &str, sts_name: &str) -> String {
    format!("{pvc_name}-{sts_name}-0")
}

async fn update_pvc(
    pvc_api: &Api<PersistentVolumeClaim>,
    pvc_full_name: &str,
    value: Quantity,
) -> Result<(), Error> {
    info!("Updating PVC {}, Value: {:?}", pvc_full_name, value);
    let mut pvc_requests: BTreeMap<String, Quantity> = BTreeMap::new();
    pvc_requests.insert("storage".to_string(), value);

    let mut pvc = pvc_api.get(pvc_full_name).await?;

    pvc.metadata.managed_fields = None;

    pvc.spec = Some(PersistentVolumeClaimSpec {
        access_modes: Some(vec!["ReadWriteOnce".to_owned()]),
        resources: Some(ResourceRequirements {
            limits: None,
            requests: Some(pvc_requests),
        }),
        ..PersistentVolumeClaimSpec::default()
    });

    let patch_params: PatchParams = PatchParams {
        dry_run: false,
        force: true,
        field_manager: Some("cntrlr".to_string()),
        field_validation: None,
    };

    let _o = pvc_api
        .patch(pvc_full_name, &patch_params, &Patch::Apply(pvc))
        .await
        .map_err(Error::KubeError);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{stateful_set_from_cdb, StatefulSet};
    use crate::apis::coredb_types::{CoreDB, CoreDBSpec};
    use kube::Resource;

    #[test]
    fn test_user_specified_uid() {
        let mut cdb_spec: CoreDBSpec = CoreDBSpec::default();
        cdb_spec.uid = 1000;
        let mut coredb: CoreDB = CoreDB::new("check-uid", cdb_spec);

        coredb.meta_mut().namespace = Some("default".into());
        coredb.meta_mut().uid = Some("752d59ef-2671-4890-9feb-0097459b18c8".into());
        let sts: StatefulSet = stateful_set_from_cdb(&coredb);

        assert_eq!(
            sts.spec
                .expect("StatefulSet does not have a spec")
                .template
                .spec
                .expect("Did not have a pod spec")
                .containers[0]
                .clone()
                .security_context
                .expect("Did not have a security context")
                .run_as_user
                .unwrap(),
            1000
        );
    }
}
