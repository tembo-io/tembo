use crate::{Context, CoreDB, Error, Result};
use k8s_openapi::{
    api::{
        apps::v1::{StatefulSet, StatefulSetSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, PersistentVolumeClaim, PersistentVolumeClaimSpec, PodSpec,
            PodTemplateSpec, ResourceRequirements,
        },
    },
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector},
};
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams, ResourceExt},
    Resource,
};
use std::{collections::BTreeMap, sync::Arc};

pub async fn reconcile_sts(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = cdb.name_any();
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let mut pvc_requests: BTreeMap<String, Quantity> = BTreeMap::new();
    let sts_api: Api<StatefulSet> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "coredb".to_owned());
    pvc_requests.insert("storage".to_string(), Quantity("8Gi".to_string()));

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
                    containers: vec![Container {
                        env: Option::from(vec![EnvVar {
                            name: "POSTGRES_PASSWORD".to_owned(),
                            value: Some("password".to_owned()),
                            value_from: None,
                        }]),
                        name: name.to_owned(),
                        image: Some(cdb.spec.image.clone()),
                        ports: Some(vec![ContainerPort {
                            container_port: 5432,
                            ..ContainerPort::default()
                        }]),
                        ..Container::default()
                    }],
                    ..PodSpec::default()
                }),
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..ObjectMeta::default()
                }),
            },
            volume_claim_templates: Option::from(vec![PersistentVolumeClaim {
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

    let ps = PatchParams::apply("cntrlr").force();
    let _o = sts_api
        .patch(&name, &ps, &Patch::Apply(&sts))
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}
