use crate::{Context, CoreDB, Error, Result};
use k8s_openapi::{
    api::{
        apps::v1::{StatefulSet, StatefulSetSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, EnvVarSource, ExecAction, PersistentVolumeClaim,
            PersistentVolumeClaimSpec, PodSpec, PodTemplateSpec, Probe, ResourceRequirements,
            SecretKeySelector, SecurityContext, VolumeMount,
        },
    },
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector},
};
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams, ResourceExt},
    Resource,
};

use std::{collections::BTreeMap, sync::Arc};

pub fn stateful_set_from_cdb(cdb: &CoreDB) -> StatefulSet {
    let ns = cdb.namespace().unwrap();
    let name = cdb.name_any();
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let mut pvc_requests: BTreeMap<String, Quantity> = BTreeMap::new();
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "coredb".to_owned());
    labels.insert("statefulset".to_owned(), name.to_owned());
    pvc_requests.insert("storage".to_string(), Quantity("8Gi".to_string()));

    let postgres_env = Some(vec![EnvVar {
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
    }]);

    let postgres_volume_mounts = Some(vec![VolumeMount {
        name: "data".to_owned(),
        mount_path: "/var/lib/postgresql/data".to_owned(),
        ..VolumeMount::default()
    }]);

    let sts: StatefulSet = StatefulSet {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(ns.to_owned()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref]),
            ..ObjectMeta::default()
        },
        spec: Some(StatefulSetSpec {
            replicas: Some(cdb.spec.replicas.clone()),
            selector: LabelSelector {
                match_expressions: None,
                match_labels: Some(labels.clone()),
            },
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    containers: vec![
                        // This container for running postgresql
                        Container {
                            env: postgres_env.clone(),
                            security_context: Some(SecurityContext {
                                run_as_user: Some(cdb.spec.uid.clone() as i64),
                                allow_privilege_escalation: Some(false),
                                ..SecurityContext::default()
                            }),
                            name: "postgres".to_string(),
                            image: Some(cdb.spec.image.clone()),
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
                    ],
                    init_containers: Option::from(vec![Container {
                        env: postgres_env.clone(),
                        name: "pg-directory-init".to_string(),
                        image: Some(cdb.spec.image.clone()),
                        volume_mounts: postgres_volume_mounts.clone(),
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
                        "
                            .to_string(),
                        ]),
                        ..Container::default()
                    }]),
                    ..PodSpec::default()
                }),
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..ObjectMeta::default()
                }),
            },
            volume_claim_templates: Some(vec![PersistentVolumeClaim {
                metadata: ObjectMeta {
                    name: Some("data".to_string()),
                    ..ObjectMeta::default()
                },
                spec: Some(PersistentVolumeClaimSpec {
                    access_modes: Some(vec!["ReadWriteOnce".to_owned()]),
                    resources: Some(ResourceRequirements {
                        limits: None,
                        requests: Some(pvc_requests),
                    }),
                    ..PersistentVolumeClaimSpec::default()
                }),
                status: None,
            }]),
            ..StatefulSetSpec::default()
        }),
        ..StatefulSet::default()
    };
    return sts;
}

pub async fn reconcile_sts(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();

    let sts: StatefulSet = stateful_set_from_cdb(cdb);

    let sts_api: Api<StatefulSet> = Api::namespaced(client, &sts.clone().metadata.namespace.unwrap());

    let ps = PatchParams::apply("cntrlr").force();
    let _o = sts_api
        .patch(&sts.clone().metadata.name.unwrap(), &ps, &Patch::Apply(&sts))
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{stateful_set_from_cdb, StatefulSet};
    use crate::{CoreDB, CoreDBSpec};
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
