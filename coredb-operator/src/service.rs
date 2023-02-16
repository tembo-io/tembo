use crate::{Context, CoreDB, Error};
use k8s_openapi::{
    api::core::v1::{Service, ServicePort, ServiceSpec},
    apimachinery::pkg::{apis::meta::v1::ObjectMeta, util::intstr::IntOrString},
};
use kube::{
    api::{Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc};

pub async fn reconcile_svc(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = cdb.name_any();
    let svc_api: Api<Service> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();

    let mut selector_labels: BTreeMap<String, String> = BTreeMap::new();
    selector_labels.insert("app".to_owned(), "coredb".to_string());
    selector_labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    let mut labels = selector_labels.clone();
    labels.insert("component".to_owned(), "postgres".to_owned());

    let svc: Service = Service {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(ns.to_owned()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref.clone()]),
            ..ObjectMeta::default()
        },
        spec: Some(ServiceSpec {
            ports: Some(vec![ServicePort {
                port: cdb.spec.port.clone(),
                ..ServicePort::default()
            }]),
            selector: Some(selector_labels.clone()),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = svc_api
        .patch(&name, &ps, &Patch::Apply(&svc))
        .await
        .map_err(Error::KubeError)?;

    let name = cdb.name_any() + "-metrics";

    if !(cdb.spec.postgresExporterEnabled) {
        // check if service exists and delete it
        let _o = svc_api.delete(&name, &Default::default()).await;
        return Ok(());
    }

    let mut selector_labels: BTreeMap<String, String> = BTreeMap::new();
    selector_labels.insert("app".to_owned(), "coredb".to_string());
    selector_labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    let mut labels = selector_labels.clone();
    labels.insert("component".to_owned(), "metrics".to_owned());

    let metrics_svc: Service = Service {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(ns.to_owned()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref]),
            ..ObjectMeta::default()
        },
        spec: Some(ServiceSpec {
            ports: Some(vec![ServicePort {
                port: 80,
                name: Some("metrics".to_string()),
                target_port: Some(IntOrString::String("metrics".to_string())),
                ..ServicePort::default()
            }]),
            selector: Some(selector_labels.clone()),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = svc_api
        .patch(&name, &ps, &Patch::Apply(&metrics_svc))
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}
