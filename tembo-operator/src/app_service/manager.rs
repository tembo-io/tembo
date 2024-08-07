use crate::{
    apis::coredb_types::CoreDB, cloudnativepg::placement::cnpg_placement::PlacementConfig,
    ingress_route_crd::IngressRouteRoutes, Context, Error, Result,
};
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            Capabilities, Container, ContainerPort, EnvVar, EnvVarSource, HTTPGetAction,
            PodSecurityContext, PodSpec, PodTemplateSpec, Probe, Secret, SecretKeySelector,
            SecretVolumeSource, SecurityContext, Service, ServicePort, ServiceSpec, Volume,
            VolumeMount,
        },
    },
    apimachinery::pkg::{
        apis::meta::v1::{LabelSelector, OwnerReference},
        util::intstr::IntOrString,
    },
    ByteString,
};
use kube::{
    api::{Api, ListParams, ObjectMeta, Patch, PatchParams, ResourceExt},
    runtime::controller::Action,
    Client, Resource,
};
use lazy_static::lazy_static;
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

use crate::{app_service::types::IngressType, secret::fetch_all_decoded_data_from_secret};

const APP_CONTAINER_PORT_PREFIX: &str = "app-";

lazy_static! {
    static ref FORWARDED_ENV_VARS: Vec<EnvVar> = {
        let mut env_vars = Vec::new();
        for (key, value) in std::env::vars() {
            if key.starts_with("TEMBO_APPS_DEFAULT_ENV_") {
                let new_key = key.replace("TEMBO_APPS_DEFAULT_ENV_", "TEMBO_");
                env_vars.push(EnvVar {
                    name: new_key,
                    value: Some(value),
                    ..EnvVar::default()
                });
            }
        }
        env_vars
    };
}

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
    podmonitor: Option<podmon::PodMonitor>,
}

// generates Kubernetes Deployment and Service templates for a AppService
fn generate_resource(
    appsvc: &AppService,
    coredb_name: &str,
    namespace: &str,
    oref: OwnerReference,
    domain: Option<String>,
    annotations: &BTreeMap<String, String>,
    placement: Option<PlacementConfig>,
) -> AppServiceResources {
    let resource_name = format!("{}-{}", coredb_name, appsvc.name.clone());
    let service = appsvc.routing.as_ref().map(|_| {
        generate_service(
            appsvc,
            coredb_name,
            &resource_name,
            namespace,
            oref.clone(),
            annotations,
        )
    });
    let deployment = generate_deployment(
        appsvc,
        coredb_name,
        &resource_name,
        namespace,
        oref.clone(),
        annotations,
        placement.clone(),
    );

    let maybe_podmonitor = generate_podmonitor(appsvc, &resource_name, namespace, annotations);

    // If DATA_PLANE_BASEDOMAIN is not set, don't generate IngressRoutes, IngressRouteTCPs, or EntryPoints
    if domain.is_none() {
        return AppServiceResources {
            deployment,
            name: resource_name,
            service,
            ingress_routes: None,
            ingress_tcp_routes: None,
            entry_points: None,
            entry_points_tcp: None,
            podmonitor: maybe_podmonitor,
        };
    }
    // It's safe to unwrap domain here because we've already checked if it's None
    let host_matcher = format!(
        "Host(`{subdomain}.{domain}`)",
        subdomain = coredb_name,
        domain = domain.clone().unwrap()
    );
    let ingress_routes = generate_ingress_routes(
        appsvc,
        &resource_name,
        namespace,
        host_matcher.clone(),
        coredb_name,
    );

    let host_matcher_tcp = format!(
        "HostSNI(`{subdomain}.{domain}`)",
        subdomain = coredb_name,
        domain = domain.unwrap()
    );

    let ingress_tcp_routes = generate_ingress_tcp_routes(
        appsvc,
        &resource_name,
        namespace,
        host_matcher_tcp,
        coredb_name,
    );
    // fetch entry points where ingress type is http
    let entry_points: Option<Vec<String>> = appsvc.routing.as_ref().map(|routes| {
        routes
            .iter()
            .filter_map(|route| {
                if route.ingress_type == Some(IngressType::http) {
                    route.entry_points.clone()
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    });

    // fetch tcp entry points where ingress type is tcp
    let entry_points_tcp: Option<Vec<String>> = appsvc.routing.as_ref().map(|routes| {
        routes
            .iter()
            .filter_map(|route| {
                if route.ingress_type == Some(IngressType::tcp) {
                    route.entry_points.clone()
                } else {
                    None
                }
            })
            .flatten()
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
        podmonitor: maybe_podmonitor,
    }
}

// templates the Kubernetes Service for an AppService
fn generate_service(
    appsvc: &AppService,
    coredb_name: &str,
    resource_name: &str,
    namespace: &str,
    oref: OwnerReference,
    annotations: &BTreeMap<String, String>,
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
                    name: Some(format!("{APP_CONTAINER_PORT_PREFIX}{p}")),
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
            annotations: Some(annotations.clone()),
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
    annotations: &BTreeMap<String, String>,
    placement: Option<PlacementConfig>,
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
        annotations: Some(annotations.clone()),
        ..ObjectMeta::default()
    };

    let (readiness_probe, liveness_probe) = match appsvc.probes.clone() {
        Some(probes) => {
            let readiness_probe = Probe {
                http_get: Some(HTTPGetAction {
                    path: Some(probes.readiness.path),
                    port: IntOrString::Int(probes.readiness.port),
                    ..HTTPGetAction::default()
                }),
                initial_delay_seconds: Some(probes.readiness.initial_delay_seconds as i32),
                ..Probe::default()
            };
            let liveness_probe = Probe {
                http_get: Some(HTTPGetAction {
                    path: Some(probes.liveness.path),
                    port: IntOrString::Int(probes.liveness.port),
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
                name: Some(format!("{APP_CONTAINER_PORT_PREFIX}{p}")),
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

    // ensure hyphen in env var name (cdb name allows hyphen)
    let cdb_name_env = coredb_name.to_uppercase().replace('-', "_");

    // map postgres connection secrets to env vars
    // mapping directly to env vars instead of using a SecretEnvSource
    // so that we can select which secrets to map into appService
    // generally, the system roles (e.g. postgres-exporter role) should not be injected to the appService
    // these three are the only secrets that are mapped into the container
    let r_conn = format!("{}_R_CONNECTION", cdb_name_env);
    let ro_conn = format!("{}_RO_CONNECTION", cdb_name_env);
    let rw_conn = format!("{}_RW_CONNECTION", cdb_name_env);
    let apps_connection_secret_name = format!("{}-apps", coredb_name);

    // map the secrets we inject to appService containers
    let default_app_envs = vec![
        EnvVar {
            name: r_conn,
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: Some(apps_connection_secret_name.clone()),
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
                    name: Some(apps_connection_secret_name.clone()),
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
                    name: Some(apps_connection_secret_name.clone()),
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
                                name: Some(apps_connection_secret_name.clone()),
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

    // Check for tembo.io/instance_id and tembo.io/organization_id annotations
    if let Some(instance_id) = annotations.get("tembo.io/instance_id") {
        env_vars.push(EnvVar {
            name: "TEMBO_INSTANCE_ID".to_string(),
            value: Some(instance_id.clone()),
            ..EnvVar::default()
        });
    }

    if let Some(organization_id) = annotations.get("tembo.io/organization_id") {
        env_vars.push(EnvVar {
            name: "TEMBO_ORG_ID".to_string(),
            value: Some(organization_id.clone()),
            ..EnvVar::default()
        });
    }

    // Add the pre-loaded forwarded environment variables
    env_vars.extend(FORWARDED_ENV_VARS.iter().cloned());

    // combine the secret env vars and those provided in spec by user
    env_vars.extend(default_app_envs);

    // Create volume vec and add certs volume from secret
    let mut volumes: Vec<Volume> = Vec::new();
    let mut volume_mounts: Vec<VolumeMount> = Vec::new();

    // If USE_SHARED_CA is not set, we don't need to mount the certs
    match std::env::var("USE_SHARED_CA") {
        Ok(_) => {
            // Create volume and add it to volumes vec
            let certs_volume = Volume {
                name: "tembo-certs".to_string(),
                secret: Some(SecretVolumeSource {
                    secret_name: Some(format!("{}-server1", coredb_name)),
                    ..SecretVolumeSource::default()
                }),
                ..Volume::default()
            };
            volumes.push(certs_volume);

            // Create volume mounts vec and add certs volume mount
            let certs_volume_mount = VolumeMount {
                name: "tembo-certs".to_string(),
                mount_path: "/tembo/certs".to_string(),
                read_only: Some(true),
                ..VolumeMount::default()
            };
            volume_mounts.push(certs_volume_mount);
        }
        Err(_) => {
            warn!("USE_SHARED_CA not set, skipping certs volume mount");
        }
    }

    let mut pod_security_context: Option<PodSecurityContext> = None;
    // Add any user provided volumes / volume mounts
    if let Some(storage) = appsvc.storage.clone() {
        // when there are user specified volumes, we need to let kubernetes modify permissions of those volumes
        pod_security_context = Some(PodSecurityContext {
            fs_group: Some(65534),
            ..PodSecurityContext::default()
        });
        if let Some(vols) = storage.volumes {
            volumes.extend(vols);
        }
        if let Some(vols) = storage.volume_mounts {
            volume_mounts.extend(vols);
        }
    }

    let affinity = placement.as_ref().and_then(|p| p.combine_affinity_items());
    let node_selector = placement.as_ref().and_then(|p| p.node_selector.clone());
    let tolerations = placement.as_ref().map(|p| p.tolerations.clone());
    let topology_spread_constraints = placement
        .as_ref()
        .and_then(|p| p.topology_spread_constraints.clone());
    let image_link = format!("{}/{}", appsvc.repository, appsvc.image);

    let pod_spec = PodSpec {
        affinity,
        containers: vec![Container {
            args: appsvc.args.clone(),
            command: appsvc.command.clone(),
            env: Some(env_vars),
            image: Some(image_link),
            name: appsvc.name.clone(),
            ports: container_ports,
            resources: Some(appsvc.resources.clone()),
            readiness_probe,
            liveness_probe,
            security_context: Some(security_context),
            volume_mounts: Some(volume_mounts),
            ..Container::default()
        }],
        node_selector,
        tolerations,
        topology_spread_constraints,
        volumes: Some(volumes),
        security_context: pod_security_context,
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
    let label_selector = format!(
        "component={},coredb.io/name={}",
        COMPONENT_NAME, coredb_name
    );
    let deployent_api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().labels(&label_selector).timeout(10);
    let deployments = deployent_api.list(&lp).await.map_err(Error::KubeError)?;
    Ok(deployments
        .items
        .iter()
        .filter_map(|d| d.metadata.name.clone())
        .collect())
}

/// Retrieves all AppService component Deployments in the namespace
///
/// This function should return all available deployments with an AppService label
/// and return the actual Deployment struct for each as a vector. This allows us
/// to use the full current state of the deployment rather than simply the name.
pub async fn get_appservice_deployment_objects(
    client: &Client,
    namespace: &str,
    coredb_name: &str,
) -> Result<Vec<Deployment>, Error> {
    let label_selector = format!(
        "component={},coredb.io/name={}",
        COMPONENT_NAME, coredb_name
    );
    let deployent_api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().labels(&label_selector).timeout(10);
    let deployments = deployent_api.list(&lp).await.map_err(Error::KubeError)?;
    Ok(deployments.items)
}

// gets all names of AppService Services in the namespace
// that have the label "component=AppService" and belong to the coredb
async fn get_appservice_services(
    client: &Client,
    namespace: &str,
    coredb_name: &str,
) -> Result<Vec<String>, Error> {
    let label_selector = format!(
        "component={},coredb.io/name={}",
        COMPONENT_NAME, coredb_name
    );
    let deployent_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().labels(&label_selector).timeout(10);
    let services = deployent_api.list(&lp).await.map_err(Error::KubeError)?;
    Ok(services
        .items
        .iter()
        .filter_map(|d| d.metadata.name.clone())
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

        let service_api: Api<Service> = Api::namespaced(client.clone(), ns);
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

        let podmon_api: Api<podmon::PodMonitor> = Api::namespaced(client.clone(), ns);
        if let Some(mut pmon) = res.podmonitor {
            // assign ownership of the PodMonitor to the Service
            // if Service is deleted, so is the PodMonitor
            let meta = service_api.get(&res.name).await;
            if let Ok(svc) = meta {
                let uid = svc.metadata.uid.unwrap_or_default();
                let oref = OwnerReference {
                    api_version: "v1".to_string(),
                    kind: "Service".to_string(),
                    name: res.name.clone(),
                    uid,
                    controller: Some(true),
                    block_owner_deletion: Some(true),
                };
                pmon.metadata.owner_references = Some(vec![oref]);
            }
            match podmon_api
                .patch(&res.name, &ps, &Patch::Apply(&pmon))
                .await
                .map_err(Error::KubeError)
            {
                Ok(_) => {
                    debug!("ns: {}, applied PodMonitor: {}", ns, res.name);
                }
                Err(e) => {
                    has_errors = true;
                    error!(
                        "ns: {}, failed to apply PodMonitor for AppService: {}, error: {}",
                        ns, res.name, e
                    );
                }
            }
        } else {
            match podmon_api.delete(&res.name, &Default::default()).await.ok() {
                Some(_) => {
                    debug!("ns: {}, deleted PodMonitor: {}", ns, res.name);
                }
                None => {
                    debug!("ns: {}, PodMonitor does not exist: {}", ns, res.name);
                }
            }
        }
    }
    has_errors
}

// generate_appsvc_annotations generates the annotations for the AppService resources
fn generate_appsvc_annotations(cdb: &CoreDB) -> BTreeMap<String, String> {
    cdb.metadata.annotations.as_ref().map_or_else(
        || {
            debug!(
                "failed to generate annotations for AppService: {}, error: No annotations found",
                cdb.name_any()
            );
            BTreeMap::new()
        },
        |annotations| {
            annotations
                .iter()
                .map(|(k, v)| {
                    if k == "tembo.io/org_id" {
                        // Change key to "tembo.io/organization_id" if it matches "tembo.io/org_id"
                        ("tembo.io/organization_id".to_string(), v.clone())
                    } else {
                        // Otherwise, clone the key and value as is
                        (k.clone(), v.clone())
                    }
                })
                .collect()
        },
    )
}

pub async fn reconcile_app_services(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    placement: Option<PlacementConfig>,
) -> Result<(), Action> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let coredb_name = cdb.name_any();
    let oref = cdb.controller_owner_ref(&()).unwrap();
    let deployment_api: Api<Deployment> = Api::namespaced(client.clone(), &ns);
    let service_api: Api<Service> = Api::namespaced(client.clone(), &ns);

    // Generate labels to attach to the AppService resources
    let annotations = generate_appsvc_annotations(cdb);

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

    match prepare_apps_connection_secret(ctx.client.clone(), cdb).await {
        Ok(_) => {}
        Err(_) => {
            error!(
                "Failed to prepare Apps Connection Secret for CoreDB: {}",
                coredb_name
            );
            return Err(Action::requeue(Duration::from_secs(300)));
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
                    error!(
                        "ns: {}, Failed to delete AppService: {}, error: {}",
                        ns, d, e
                    );
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
                    error!(
                        "ns: {}, Failed to delete AppService: {}, error: {}",
                        ns, d, e
                    );
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
        Ok(domain) => Some(domain),
        Err(_) => {
            warn!("DATA_PLANE_BASEDOMAIN not set, skipping ingress reconciliation");
            None
        }
    };
    // Iterate over each AppService and process routes
    let resources: Vec<AppServiceResources> = appsvcs
        .iter()
        .map(|appsvc| {
            generate_resource(
                appsvc,
                &coredb_name,
                &ns,
                oref.clone(),
                domain.to_owned(),
                &annotations,
                placement.clone(),
            )
        })
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

    // Only reconcile IngressRoute and IngressRouteTCP if DATA_PLANE_BASEDOMAIN is set
    if domain.is_some() {
        match reconcile_ingress(
            client.clone(),
            &coredb_name,
            &ns,
            oref.clone(),
            desired_routes,
            desired_middlewares.clone(),
            desired_entry_points,
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

        for appsvc in appsvcs.iter() {
            let app_name = appsvc.name.clone();

            match reconcile_ingress_tcp(
                client.clone(),
                &coredb_name,
                &ns,
                oref.clone(),
                desired_tcp_routes.clone(),
                // TODO: fill with actual MiddlewareTCPs when it is supported
                // first supported MiddlewareTCP will be for custom domains
                vec![],
                desired_entry_points_tcp.clone(),
                &app_name,
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
        }
    }
    if has_errors || apply_errored {
        return Err(Action::requeue(Duration::from_secs(300)));
    }
    Ok(())
}

pub async fn prepare_apps_connection_secret(client: Client, cdb: &CoreDB) -> Result<(), Error> {
    let namespace = cdb.namespace().unwrap();
    let cdb_name = cdb.metadata.name.clone().unwrap();
    let secret_name = format!("{}-connection", cdb_name);
    let new_secret_name = format!("{}-apps", cdb_name);

    let secrets_api: Api<Secret> = Api::namespaced(client.clone(), &namespace);

    // Fetch the original secret
    let original_secret_data =
        fetch_all_decoded_data_from_secret(secrets_api.clone(), secret_name.to_string()).await?;

    // Modify the secret data
    let mut new_secret_data = BTreeMap::new();
    for (key, value) in original_secret_data {
        match key.as_str() {
            "r_uri" | "ro_uri" | "rw_uri" => {
                let new_value = format!("{}?application_name=tembo-apps", value);
                new_secret_data.insert(key, new_value);
            }
            _ => {}
        };
    }

    // Encode the modified secret data
    let encoded_secret_data: BTreeMap<String, ByteString> = new_secret_data
        .into_iter()
        .map(|(k, v)| (k, ByteString(v.into_bytes())))
        .collect();

    // Create a new secret with the modified data
    let new_secret = Secret {
        data: Some(encoded_secret_data),
        metadata: kube::api::ObjectMeta {
            name: Some(new_secret_name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    // Apply the new secret
    let patch_params = PatchParams::apply("cntrlr").force();
    secrets_api
        .patch(&new_secret_name, &patch_params, &Patch::Apply(&new_secret))
        .await?;

    Ok(())
}

use crate::prometheus::podmonitor_crd as podmon;

fn generate_podmonitor(
    appsvc: &AppService,
    resource_name: &str,
    namespace: &str,
    annotations: &BTreeMap<String, String>,
) -> Option<podmon::PodMonitor> {
    let metrics = appsvc.metrics.clone()?;

    let mut selector_labels: BTreeMap<String, String> = BTreeMap::new();
    selector_labels.insert("app".to_owned(), resource_name.to_string());

    let mut labels = selector_labels.clone();
    labels.insert("component".to_owned(), COMPONENT_NAME.to_owned());
    labels.insert("coredb.io/name".to_owned(), namespace.to_owned());

    let podmon_metadata = ObjectMeta {
        name: Some(resource_name.to_string()),
        namespace: Some(namespace.to_owned()),
        labels: Some(labels.clone()),
        annotations: Some(annotations.clone()),
        ..ObjectMeta::default()
    };

    let metrics_endpoint = podmon::PodMonitorPodMetricsEndpoints {
        path: Some(metrics.path),
        port: Some(format!("{APP_CONTAINER_PORT_PREFIX}{}", metrics.port)),
        ..podmon::PodMonitorPodMetricsEndpoints::default()
    };

    let pmonspec = podmon::PodMonitorSpec {
        pod_metrics_endpoints: Some(vec![metrics_endpoint]),
        selector: podmon::PodMonitorSelector {
            match_labels: Some(selector_labels.clone()),
            ..podmon::PodMonitorSelector::default()
        },
        ..podmon::PodMonitorSpec::default()
    };
    Some(podmon::PodMonitor {
        metadata: podmon_metadata,
        spec: pmonspec,
    })
}

#[cfg(test)]
mod tests {
    use crate::{apis::coredb_types::CoreDB, app_service::manager::generate_appsvc_annotations};
    use std::collections::BTreeMap;

    #[test]
    fn test_generate_appsvc_annotations() {
        // Create a CoreDB object
        let cdb_yaml = r#"
            apiVersion: coredb.io/v1alpha1
            kind: CoreDB
            metadata:
              name: test
              namespace: default
              annotations:
                tembo.io/data_plane_id: org_jQ7nBcX8uPzLkYdGtW1fvHOqMRST
                tembo.io/entity_name: VectorDB
                tembo.io/instance_id: inst_4836271985012_bZTnPq_85
                tembo.io/org_id: org_jQ7nBcX8uPzLkYdGtW1fvHOqMRST
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
        let coredb: CoreDB = serde_yaml::from_str(cdb_yaml).expect("Failed to parse YAML");

        let annotataions = generate_appsvc_annotations(&coredb);

        // Create the expected labels
        let expected_annotations: BTreeMap<String, String> = vec![
            (
                "tembo.io/data_plane_id".to_string(),
                "org_jQ7nBcX8uPzLkYdGtW1fvHOqMRST".to_string(),
            ),
            ("tembo.io/entity_name".to_string(), "VectorDB".to_string()),
            (
                "tembo.io/instance_id".to_string(),
                "inst_4836271985012_bZTnPq_85".to_string(),
            ),
            (
                "tembo.io/organization_id".to_string(),
                "org_jQ7nBcX8uPzLkYdGtW1fvHOqMRST".to_string(),
            ),
        ]
        .into_iter()
        .collect();

        // Assert that the generated labels match the expected labels
        assert_eq!(annotataions, expected_annotations);
    }
}
