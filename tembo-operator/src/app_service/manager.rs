use crate::{apis::coredb_types::CoreDB, ingress_route_crd::IngressRouteRoutes, Context, Error, Result};
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            Capabilities, Container, ContainerPort, EnvVar, EnvVarSource, HTTPGetAction, PodSpec,
            PodTemplateSpec, Probe, SecretKeySelector, SecurityContext, Service, ServicePort, ServiceSpec,
        },
    },
    apimachinery::pkg::{
        apis::meta::v1::{LabelSelector, OwnerReference},
        util::intstr::IntOrString,
    },
};
use kube::{
    api::{Api, ListParams, ObjectMeta, Patch, PatchParams, ResourceExt},
    runtime::controller::Action,
    Client, Resource,
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

use crate::{
    app_service::ingress::{generate_ingress_tcp_routes, reconcile_ingress_tcp},
    traefik::ingress_route_tcp_crd::IngressRouteTCPRoutes,
};
use tracing::{debug, error, warn};

use super::{
    ingress::{generate_ingress_routes, reconcile_ingress},
    types::{AppService, EnvVarRef, Middleware, COMPONENT_NAME},
};

// private wrapper to hold the AppService Resources
#[derive(Clone, Debug)]
struct AppServiceResources {
    deployment: Deployment,
    name: String,
    service: Option<Service>,
    ingress_routes: Option<Vec<IngressRouteRoutes>>,
    ingress_tcp_routes: Option<Vec<IngressRouteTCPRoutes>>,
    entry_points: Option<Vec<String>>,
    entry_points_tcp: Option<Vec<String>>,
}

// generates Kubernetes Deployment and Service templates for a AppService
fn generate_resource(
    appsvc: &AppService,
    coredb_name: &str,
    namespace: &str,
    oref: OwnerReference,
    domain: String,
) -> AppServiceResources {
    let resource_name = format!("{}-{}", coredb_name, appsvc.name.clone());
    let service = appsvc
        .routing
        .as_ref()
        .map(|_| generate_service(appsvc, coredb_name, &resource_name, namespace, oref.clone()));
    let deployment = generate_deployment(appsvc, coredb_name, &resource_name, namespace, oref);

    let host_matcher = format!(
        "Host(`{subdomain}.{domain}`)",
        subdomain = coredb_name,
        domain = domain
    );
    let ingress_routes = generate_ingress_routes(
        appsvc,
        &resource_name,
        namespace,
        host_matcher.clone(),
        coredb_name,
    );
    let ingress_tcp_routes =
        generate_ingress_tcp_routes(appsvc, &resource_name, namespace, host_matcher, coredb_name);
    // fetch entry points from routing
    let entry_points: Option<Vec<String>> = appsvc.routing.as_ref().map(|routes| {
        routes
            .iter()
            .filter_map(|route| route.entry_points.clone())
            .flatten()
            .collect()
    });
    // fetch tcp entry points where entrypoint is ferretdb
    let entry_points_tcp: Option<Vec<String>> = appsvc.routing.as_ref().map(|routes| {
        routes
            .iter()
            .filter_map(|route| {
                if route
                    .entry_points
                    .clone()
                    .unwrap_or_default()
                    .contains(&"ferretdb".to_string())
                {
                    Some("ferretdb".to_string())
                } else {
                    None
                }
            })
            .collect()
    });

    AppServiceResources {
        deployment,
        name: resource_name,
        service,
        ingress_routes,
        ingress_tcp_routes,
        entry_points,
        entry_points_tcp,
    }
}

// templates the Kubernetes Service for an AppService
fn generate_service(
    appsvc: &AppService,
    coredb_name: &str,
    resource_name: &str,
    namespace: &str,
    oref: OwnerReference,
) -> Service {
    let mut selector_labels: BTreeMap<String, String> = BTreeMap::new();

    selector_labels.insert("app".to_owned(), resource_name.to_string());
    selector_labels.insert("component".to_owned(), COMPONENT_NAME.to_string());
    selector_labels.insert("coredb.io/name".to_owned(), coredb_name.to_string());

    let mut labels = selector_labels.clone();
    labels.insert("component".to_owned(), COMPONENT_NAME.to_owned());

    let ports = match appsvc.routing.as_ref() {
        Some(routing) => {
            // de-dupe any ports because we can have multiple appService routing configs for the same port
            // but we only need one ServicePort per port
            let distinct_ports = routing
                .iter()
                .map(|r| r.port)
                .collect::<std::collections::HashSet<u16>>();

            let ports: Vec<ServicePort> = distinct_ports
                .into_iter()
                .map(|p| ServicePort {
                    port: p as i32,
                    // there can be more than one ServicePort per Service
                    // these must be unique, so we'll use the port number
                    name: Some(format!("http-{}", p)),
                    target_port: None,
                    ..ServicePort::default()
                })
                .collect();
            Some(ports)
        }
        None => None,
    };
    Service {
        metadata: ObjectMeta {
            name: Some(resource_name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref]),
            ..ObjectMeta::default()
        },
        spec: Some(ServiceSpec {
            ports,
            selector: Some(selector_labels.clone()),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    }
}

// templates a single Kubernetes Deployment for an AppService
fn generate_deployment(
    appsvc: &AppService,
    coredb_name: &str,
    resource_name: &str,
    namespace: &str,
    oref: OwnerReference,
) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), resource_name.to_string());
    labels.insert("component".to_owned(), COMPONENT_NAME.to_string());
    labels.insert("coredb.io/name".to_owned(), coredb_name.to_string());

    let deployment_metadata = ObjectMeta {
        name: Some(resource_name.to_string()),
        namespace: Some(namespace.to_owned()),
        labels: Some(labels.clone()),
        owner_references: Some(vec![oref]),
        ..ObjectMeta::default()
    };

    let (readiness_probe, liveness_probe) = match appsvc.probes.clone() {
        Some(probes) => {
            let readiness_probe = Probe {
                http_get: Some(HTTPGetAction {
                    path: Some(probes.readiness.path),
                    port: IntOrString::String(probes.readiness.port),
                    ..HTTPGetAction::default()
                }),
                initial_delay_seconds: Some(probes.readiness.initial_delay_seconds as i32),
                ..Probe::default()
            };
            let liveness_probe = Probe {
                http_get: Some(HTTPGetAction {
                    path: Some(probes.liveness.path),
                    port: IntOrString::String(probes.liveness.port),
                    ..HTTPGetAction::default()
                }),
                initial_delay_seconds: Some(probes.liveness.initial_delay_seconds as i32),
                ..Probe::default()
            };
            (Some(readiness_probe), Some(liveness_probe))
        }
        None => (None, None),
    };

    // container ports
    let container_ports = if let Some(routings) = appsvc.routing.as_ref() {
        let distinct_ports = routings
            .iter()
            .map(|r| r.port)
            .collect::<std::collections::HashSet<u16>>();
        let container_ports: Vec<ContainerPort> = distinct_ports
            .into_iter()
            .map(|p| ContainerPort {
                container_port: p as i32,
                protocol: Some("TCP".to_string()),
                ..ContainerPort::default()
            })
            .collect();
        Some(container_ports)
    } else {
        None
    };

    // https://tembo.io/docs/tembo-cloud/security/#tenant-isolation
    // These configs are the same as CNPG configs
    let security_context = SecurityContext {
        run_as_user: Some(65534),
        allow_privilege_escalation: Some(false),
        capabilities: Some(Capabilities {
            drop: Some(vec!["ALL".to_string()]),
            ..Capabilities::default()
        }),
        privileged: Some(false),
        run_as_non_root: Some(true),
        // This part maybe we disable if we need
        // or we can mount ephemeral or persistent
        // volumes if we need to write somewhere
        read_only_root_filesystem: Some(true),
        ..SecurityContext::default()
    };

    // ensure hyphen in in env var name (cdb name allows hyphen)
    let cdb_name_env = coredb_name.to_uppercase().replace('-', "_");

    // map postgres connection secrets to env vars
    // mapping directly to env vars instead of using a SecretEnvSource
    // so that we can select which secrets to map into appService
    // generally, the system roles (e.g. postgres-exporter role) should not be injected to the appService
    // these three are the only secrets that are mapped into the container
    let r_conn = format!("{}_R_CONNECTION", cdb_name_env);
    let ro_conn = format!("{}_RO_CONNECTION", cdb_name_env);
    let rw_conn = format!("{}_RW_CONNECTION", cdb_name_env);

    // map the secrets we inject to appService containers
    let secret_envs = vec![
        EnvVar {
            name: r_conn,
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: Some(format!("{}-connection", coredb_name)),
                    key: "r_uri".to_string(),
                    ..SecretKeySelector::default()
                }),
                ..EnvVarSource::default()
            }),
            ..EnvVar::default()
        },
        EnvVar {
            name: ro_conn,
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: Some(format!("{}-connection", coredb_name)),
                    key: "ro_uri".to_string(),
                    ..SecretKeySelector::default()
                }),
                ..EnvVarSource::default()
            }),
            ..EnvVar::default()
        },
        EnvVar {
            name: rw_conn,
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: Some(format!("{}-connection", coredb_name)),
                    key: "rw_uri".to_string(),
                    ..SecretKeySelector::default()
                }),
                ..EnvVarSource::default()
            }),
            ..EnvVar::default()
        },
    ];

    // map the user provided env vars
    // users can map certain secrets to env vars of their choice
    let mut env_vars: Vec<EnvVar> = Vec::new();
    if let Some(envs) = appsvc.env.clone() {
        for env in envs {
            let evar: Option<EnvVar> = match (env.value, env.value_from_platform) {
                // Value provided
                (Some(e), _) => Some(EnvVar {
                    name: env.name,
                    value: Some(e),
                    ..EnvVar::default()
                }),
                // EnvVarRef provided, and no Value
                (None, Some(e)) => {
                    let secret_key = match e {
                        EnvVarRef::ReadOnlyConnection => "ro_uri",
                        EnvVarRef::ReadWriteConnection => "rw_uri",
                    };
                    Some(EnvVar {
                        name: env.name,
                        value_from: Some(EnvVarSource {
                            secret_key_ref: Some(SecretKeySelector {
                                name: Some(format!("{}-connection", coredb_name)),
                                key: secret_key.to_string(),
                                ..SecretKeySelector::default()
                            }),
                            ..EnvVarSource::default()
                        }),
                        ..EnvVar::default()
                    })
                }
                // everything missing, skip it
                _ => {
                    error!(
                        "ns: {}, AppService: {}, env var: {} is missing value or valueFromPlatform",
                        namespace, resource_name, env.name
                    );
                    None
                }
            };
            if let Some(e) = evar {
                env_vars.push(e);
            }
        }
    }
    // combine the secret env vars and those provided in spec by user
    env_vars.extend(secret_envs);

    let pod_spec = PodSpec {
        containers: vec![Container {
            args: appsvc.args.clone(),
            command: appsvc.command.clone(),
            env: Some(env_vars),
            image: Some(appsvc.image.clone()),
            name: appsvc.name.clone(),
            ports: container_ports,
            resources: Some(appsvc.resources.clone()),
            readiness_probe,
            liveness_probe,
            security_context: Some(security_context),
            volume_mounts: appsvc.storage.clone().and_then(|s| s.volume_mounts),
            ..Container::default()
        }],
        volumes: appsvc.storage.clone().and_then(|s| s.volumes),
        ..PodSpec::default()
    };

    let pod_template_spec = PodTemplateSpec {
        metadata: Some(deployment_metadata.clone()),
        spec: Some(pod_spec),
    };

    let deployment_spec = DeploymentSpec {
        selector: LabelSelector {
            match_labels: Some(labels.clone()),
            ..LabelSelector::default()
        },
        template: pod_template_spec,
        ..DeploymentSpec::default()
    };
    Deployment {
        metadata: deployment_metadata,
        spec: Some(deployment_spec),
        ..Deployment::default()
    }
}

// gets all names of AppService Deployments in the namespace that have the label "component=AppService"
async fn get_appservice_deployments(
    client: &Client,
    namespace: &str,
    coredb_name: &str,
) -> Result<Vec<String>, Error> {
    let label_selector = format!("component={},coredb.io/name={}", COMPONENT_NAME, coredb_name);
    let deployent_api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().labels(&label_selector).timeout(10);
    let deployments = deployent_api.list(&lp).await.map_err(Error::KubeError)?;
    Ok(deployments
        .items
        .iter()
        .map(|d| d.metadata.name.to_owned().expect("no name on resource"))
        .collect())
}

// gets all names of AppService Services in the namespace
// that have the label "component=AppService" and belong to the coredb
async fn get_appservice_services(
    client: &Client,
    namespace: &str,
    coredb_name: &str,
) -> Result<Vec<String>, Error> {
    let label_selector = format!("component={},coredb.io/name={}", COMPONENT_NAME, coredb_name);
    let deployent_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().labels(&label_selector).timeout(10);
    let services = deployent_api.list(&lp).await.map_err(Error::KubeError)?;
    Ok(services
        .items
        .iter()
        .map(|d| d.metadata.name.to_owned().expect("no name on resource"))
        .collect())
}

// determines AppService deployments
pub fn to_delete(desired: Vec<String>, actual: Vec<String>) -> Option<Vec<String>> {
    let mut to_delete: Vec<String> = Vec::new();
    for a in actual {
        // if actual not in desired, put it in the delete vev
        if !desired.contains(&a) {
            to_delete.push(a);
        }
    }
    if to_delete.is_empty() {
        None
    } else {
        Some(to_delete)
    }
}

async fn apply_resources(resources: Vec<AppServiceResources>, client: &Client, ns: &str) -> bool {
    let deployment_api: Api<Deployment> = Api::namespaced(client.clone(), ns);
    let service_api: Api<Service> = Api::namespaced(client.clone(), ns);
    let ps = PatchParams::apply("cntrlr").force();

    let mut has_errors: bool = false;

    // apply desired resources
    for res in resources {
        match deployment_api
            .patch(&res.name, &ps, &Patch::Apply(&res.deployment))
            .await
            .map_err(Error::KubeError)
        {
            Ok(_) => {
                debug!("ns: {}, applied AppService Deployment: {}", ns, res.name);
            }
            Err(e) => {
                // TODO: find a better way to handle single error without stopping all reconciliation of AppService
                has_errors = true;
                error!(
                    "ns: {}, failed to apply AppService Deployment: {}, error: {}",
                    ns, res.name, e
                );
            }
        }
        if res.service.is_none() {
            continue;
        }
        match service_api
            .patch(&res.name, &ps, &Patch::Apply(&res.service))
            .await
            .map_err(Error::KubeError)
        {
            Ok(_) => {
                debug!("ns: {}, applied AppService Service: {}", ns, res.name);
            }
            Err(e) => {
                // TODO: find a better way to handle single error without stopping all reconciliation of AppService
                has_errors = true;
                error!(
                    "ns: {}, failed to apply AppService Service: {}, error: {}",
                    ns, res.name, e
                );
            }
        }
    }
    has_errors
}

pub async fn reconcile_app_services(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let coredb_name = cdb.name_any();
    let oref = cdb.controller_owner_ref(&()).unwrap();
    let deployment_api: Api<Deployment> = Api::namespaced(client.clone(), &ns);
    let service_api: Api<Service> = Api::namespaced(client.clone(), &ns);

    let desired_deployments = match cdb.spec.app_services.clone() {
        Some(appsvcs) => appsvcs
            .iter()
            .map(|a| format!("{}-{}", coredb_name, a.name.clone()))
            .collect(),
        None => {
            debug!("No AppServices found in Instance: {}", ns);
            vec![]
        }
    };

    // only deploy the Kubernetes Service when there are routing configurations
    // we need one service per PORT, not necessarily 1 per AppService route
    let desired_services = match cdb.spec.app_services.clone() {
        Some(appsvcs) => {
            let mut desired_svc: Vec<String> = Vec::new();
            for appsvc in appsvcs.iter() {
                if appsvc.routing.as_ref().is_some() {
                    let svc_name = format!("{}-{}", coredb_name, appsvc.name);
                    desired_svc.push(svc_name.clone());
                }
            }
            desired_svc
        }
        None => {
            vec![]
        }
    };
    // TODO: we can improve our overall error handling design
    // for app_service reconciliation, not stop all reconciliation if an operation on a single AppService fails
    // however, we do want to requeue if there are any error
    // currently there are no expected errors in this path
    // for simplicity, we will return a requeue Action if there are errors
    let mut has_errors: bool = false;

    let actual_deployments = match get_appservice_deployments(&client, &ns, &coredb_name).await {
        Ok(deployments) => deployments,
        Err(e) => {
            has_errors = true;
            error!("ns: {}, failed to get AppService Deployments: {}", ns, e);
            vec![]
        }
    };
    let actual_services = match get_appservice_services(&client, &ns, &coredb_name).await {
        Ok(services) => services,
        Err(e) => {
            has_errors = true;
            error!("ns: {}, failed to get AppService Services: {}", ns, e);
            vec![]
        }
    };

    // reap any AppService Deployments that are no longer desired
    if let Some(to_delete) = to_delete(desired_deployments, actual_deployments) {
        for d in to_delete {
            match deployment_api.delete(&d, &Default::default()).await {
                Ok(_) => {
                    debug!("ns: {}, successfully deleted AppService: {}", ns, d);
                }
                Err(e) => {
                    has_errors = true;
                    error!("ns: {}, Failed to delete AppService: {}, error: {}", ns, d, e);
                }
            }
        }
    }

    // reap any AppService  that are no longer desired
    if let Some(to_delete) = to_delete(desired_services, actual_services) {
        for d in to_delete {
            match service_api.delete(&d, &Default::default()).await {
                Ok(_) => {
                    debug!("ns: {}, successfully deleted AppService: {}", ns, d);
                }
                Err(e) => {
                    has_errors = true;
                    error!("ns: {}, Failed to delete AppService: {}, error: {}", ns, d, e);
                }
            }
        }
    }

    let appsvcs = match cdb.spec.app_services.clone() {
        Some(appsvcs) => appsvcs,
        None => {
            debug!("ns: {}, No AppServices found in spec", ns);
            vec![]
        }
    };

    let domain = match std::env::var("DATA_PLANE_BASEDOMAIN") {
        Ok(domain) => domain,
        Err(_) => {
            warn!("`DATA_PLANE_BASEDOMAIN` not set -- assuming `localhost`");
            "localhost".to_string()
        }
    };
    let resources: Vec<AppServiceResources> = appsvcs
        .iter()
        .map(|appsvc| generate_resource(appsvc, &coredb_name, &ns, oref.clone(), domain.to_owned()))
        .collect();
    let apply_errored = apply_resources(resources.clone(), &client, &ns).await;

    let desired_routes: Vec<IngressRouteRoutes> = resources
        .iter()
        .filter_map(|r| r.ingress_routes.clone())
        .flatten()
        .collect();

    let desired_tcp_routes: Vec<IngressRouteTCPRoutes> = resources
        .iter()
        .filter_map(|r| r.ingress_tcp_routes.clone())
        .flatten()
        .collect();

    let desired_middlewares = appsvcs
        .iter()
        .filter_map(|appsvc| appsvc.middlewares.clone())
        .flatten()
        .collect::<Vec<Middleware>>();

    let desired_entry_points = resources
        .iter()
        .filter_map(|r| r.entry_points.clone())
        .flatten()
        .collect::<Vec<String>>();

    let desired_entry_points_tcp = resources
        .iter()
        .filter_map(|r| r.entry_points_tcp.clone())
        .flatten()
        .collect::<Vec<String>>();

    match reconcile_ingress(
        client.clone(),
        &coredb_name,
        &ns,
        oref.clone(),
        desired_routes,
        desired_middlewares.clone(),
        // desired_entry_points,
    )
    .await
    {
        Ok(_) => {
            debug!("Updated/applied IngressRoute for {}.{}", ns, coredb_name,);
        }
        Err(e) => {
            error!(
                "Failed to update/apply IngressRoute {}.{}: {}",
                ns, coredb_name, e
            );
            has_errors = true;
        }
    }

    match reconcile_ingress_tcp(
        client.clone(),
        &coredb_name,
        &ns,
        oref.clone(),
        desired_tcp_routes,
        desired_middlewares,
        desired_entry_points_tcp,
    )
    .await
    {
        Ok(_) => {
            debug!("Updated/applied IngressRouteTCP for {}.{}", ns, coredb_name,);
        }
        Err(e) => {
            error!(
                "Failed to update/apply IngressRouteTCP {}.{}: {}",
                ns, coredb_name, e
            );
            has_errors = true;
        }
    }

    if has_errors || apply_errored {
        return Err(Action::requeue(Duration::from_secs(300)));
    }
    Ok(())
}
