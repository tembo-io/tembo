use crate::{Context, CoreDB, Error};
use k8s_openapi::{
    api::core::v1::{Service, ServicePort, ServiceSpec},
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    api::{Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

pub async fn reconcile_svc(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = cdb.name_any();
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let svc_api: Api<Service> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "coredb".to_owned());

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
            selector: Some(labels.clone()),
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
                target_port: Some(IntOrString::String("metrics".to_string())),
                ..ServicePort::default()
            }]),
            selector: Some(labels.clone()),
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
