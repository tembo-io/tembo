use crate::{
    servicemonitor_crd::{
        ServiceMonitor, ServiceMonitorEndpoints, ServiceMonitorSelector, ServiceMonitorSpec,
    },
    Context, CoreDB, Error,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc};

pub async fn reconcile_servicemonitor(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = cdb.name_any();
    let servicemonitor_api: Api<ServiceMonitor> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());
    labels.insert("component".to_owned(), "metrics".to_owned());

    let servicemonitor: ServiceMonitor = ServiceMonitor {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(ns.to_owned()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref.clone()]),
            ..ObjectMeta::default()
        },
        spec: ServiceMonitorSpec {
            endpoints: vec![ServiceMonitorEndpoints {
                path: Some("/metrics".to_owned()),
                port: Some("metrics".to_string()),
                authorization: None,
                basic_auth: None,
                bearer_token_file: None,
                bearer_token_secret: None,
                enable_http2: None,
                filter_running: None,
                follow_redirects: None,
                honor_labels: None,
                honor_timestamps: None,
                interval: None,
                metric_relabelings: None,
                oauth2: None,
                params: None,
                proxy_url: None,
                relabelings: None,
                scheme: None,
                scrape_timeout: None,
                tls_config: None,
                target_port: None,
            }],
            selector: ServiceMonitorSelector {
                match_labels: Some(labels.clone()),
                match_expressions: None,
            },
            attach_metadata: None,
            job_label: None,
            label_limit: None,
            label_name_length_limit: None,
            label_value_length_limit: None,
            namespace_selector: None,
            pod_target_labels: None,
            sample_limit: None,
            target_labels: None,
            target_limit: None,
        },
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = servicemonitor_api
        .patch(&name, &ps, &Patch::Apply(&servicemonitor))
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}
