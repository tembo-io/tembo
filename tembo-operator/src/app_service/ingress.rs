use crate::{
    ingress_route_crd::{
        IngressRoute, IngressRouteRoutes, IngressRouteRoutesKind, IngressRouteRoutesMiddlewares,
        IngressRouteRoutesServices, IngressRouteRoutesServicesKind, IngressRouteSpec, IngressRouteTls,
    },
    traefik::middlewares_crd::{
        Middleware as TraefikMiddleware, MiddlewareHeaders, MiddlewareReplacePathRegex, MiddlewareSpec,
        MiddlewareStripPrefix,
    },
    Result,
};
use k8s_openapi::apimachinery::pkg::{apis::meta::v1::OwnerReference, util::intstr::IntOrString};
use kube::{
    api::{Api, ListParams, ObjectMeta, Patch, PatchParams},
    Client,
};

use std::collections::BTreeMap;

use tracing::{debug, error, warn};

use super::{
    manager::to_delete,
    types::{AppService, Middleware, COMPONENT_NAME},
};

use crate::traefik::ingress_route_tcp_crd::{
    IngressRouteTCP, IngressRouteTCPRoutes, IngressRouteTCPRoutesMiddlewares, IngressRouteTCPRoutesServices,
    IngressRouteTCPSpec, IngressRouteTCPTls,
};

#[derive(Clone, Debug)]
pub struct MiddleWareWrapper {
    pub name: String,
    pub mw: TraefikMiddleware,
}

fn generate_ingress(
    coredb_name: &str,
    namespace: &str,
    oref: OwnerReference,
    routes: Vec<IngressRouteRoutes>,
) -> IngressRoute {
    let mut selector_labels: BTreeMap<String, String> = BTreeMap::new();

    selector_labels.insert("component".to_owned(), COMPONENT_NAME.to_string());
    selector_labels.insert("coredb.io/name".to_owned(), coredb_name.to_string());

    let mut labels = selector_labels.clone();
    labels.insert("component".to_owned(), COMPONENT_NAME.to_owned());

    IngressRoute {
        metadata: ObjectMeta {
            // using coredb name, since we'll have 1x ingress per coredb
            name: Some(coredb_name.to_owned()),
            namespace: Some(namespace.to_owned()),
            owner_references: Some(vec![oref]),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: IngressRouteSpec {
            entry_points: Some(vec!["websecure".to_string()]),
            routes,
            tls: Some(IngressRouteTls::default()),
        },
    }
}

fn generate_ingress_tcp(
    coredb_name: &str,
    namespace: &str,
    oref: OwnerReference,
    routes: Vec<IngressRouteTCPRoutes>,
    entry_points: Vec<String>,
) -> IngressRouteTCP {
    let mut selector_labels: BTreeMap<String, String> = BTreeMap::new();

    selector_labels.insert("component".to_owned(), COMPONENT_NAME.to_string());
    selector_labels.insert("coredb.io/name".to_owned(), coredb_name.to_string());

    let mut labels = selector_labels.clone();
    labels.insert("component".to_owned(), COMPONENT_NAME.to_owned());

    IngressRouteTCP {
        metadata: ObjectMeta {
            // using coredb name, since we'll have 1x ingress per coredb
            name: Some(coredb_name.to_owned()),
            namespace: Some(namespace.to_owned()),
            owner_references: Some(vec![oref]),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: IngressRouteTCPSpec {
            entry_points: Some(entry_points),
            routes,
            tls: Some(IngressRouteTCPTls {
                passthrough: Some(true),
                ..IngressRouteTCPTls::default()
            }),
        },
    }
}

// creates traefik middleware objects
// named `<coredb-name>-<specified-middleware-name>`
fn generate_middlewares(
    coredb_name: &str,
    namespace: &str,
    oref: OwnerReference,
    middlewares: Vec<Middleware>,
) -> Vec<MiddleWareWrapper> {
    let mut traefik_middlwares: Vec<MiddleWareWrapper> = Vec::new();
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("component".to_owned(), COMPONENT_NAME.to_string());
    labels.insert("coredb.io/name".to_owned(), coredb_name.to_string());

    for mw in middlewares {
        let traefik_mw = match mw {
            Middleware::CustomRequestHeaders(mw) => {
                let mw_name = format!("{}-{}", coredb_name, mw.name);
                let mwh = MiddlewareHeaders {
                    custom_request_headers: Some(mw.config),
                    ..MiddlewareHeaders::default()
                };
                let tmw = TraefikMiddleware {
                    metadata: ObjectMeta {
                        name: Some(mw_name.clone()),
                        namespace: Some(namespace.to_owned()),
                        owner_references: Some(vec![oref.clone()]),
                        labels: Some(labels.clone()),
                        ..ObjectMeta::default()
                    },
                    spec: MiddlewareSpec {
                        headers: Some(mwh),
                        ..MiddlewareSpec::default()
                    },
                };
                MiddleWareWrapper {
                    name: mw_name,
                    mw: tmw,
                }
            }
            Middleware::StripPrefix(mw) => {
                let mw_name = format!("{}-{}", coredb_name, mw.name);
                let mwsp = MiddlewareStripPrefix {
                    prefixes: Some(mw.config),
                };
                let tmw = TraefikMiddleware {
                    metadata: ObjectMeta {
                        name: Some(mw_name.clone()),
                        namespace: Some(namespace.to_owned()),
                        owner_references: Some(vec![oref.clone()]),
                        labels: Some(labels.clone()),
                        ..ObjectMeta::default()
                    },
                    spec: MiddlewareSpec {
                        strip_prefix: Some(mwsp),
                        ..MiddlewareSpec::default()
                    },
                };
                MiddleWareWrapper {
                    name: mw_name,
                    mw: tmw,
                }
            }
            Middleware::ReplacePathRegex(mw) => {
                let mw_name = format!("{}-{}", coredb_name, mw.name);
                let mwrpr = MiddlewareReplacePathRegex {
                    regex: Some(mw.config.regex),
                    replacement: Some(mw.config.replacement),
                };
                let tmw = TraefikMiddleware {
                    metadata: ObjectMeta {
                        name: Some(mw_name.clone()),
                        namespace: Some(namespace.to_owned()),
                        owner_references: Some(vec![oref.clone()]),
                        labels: Some(labels.clone()),
                        ..ObjectMeta::default()
                    },
                    spec: MiddlewareSpec {
                        replace_path_regex: Some(mwrpr),
                        ..MiddlewareSpec::default()
                    },
                };
                MiddleWareWrapper {
                    name: mw_name,
                    mw: tmw,
                }
            }
        };
        traefik_middlwares.push(traefik_mw);
    }
    traefik_middlwares
}

// generates Kubernetes IngressRoute template for an appService
// maps the specified
pub fn generate_ingress_routes(
    appsvc: &AppService,
    resource_name: &str,
    namespace: &str,
    host_matcher: String,
    coredb_name: &str,
) -> Option<Vec<IngressRouteRoutes>> {
    match appsvc.routing.clone() {
        Some(routings) => {
            let mut routes: Vec<IngressRouteRoutes> = Vec::new();
            for route in routings.iter() {
                match route.ingress_path.clone() {
                    Some(path) => {
                        if route.entry_points.clone()?.contains(&"ferretdb".to_string()) {
                            // Do not create IngressRouteRoutes for ferretdb. Needs IngressRouteTCPRoute.
                            println!("Skipping IngressRouteRoutes for ferretdb");
                            println!("ENTRY POINTS: {:?}", route.entry_points.clone()?);
                            println!("ROUTE: {:?}", route);
                            continue;
                        }
                        let matcher = format!("{host_matcher} && PathPrefix(`{}`)", path);
                        let middlewares: Option<Vec<IngressRouteRoutesMiddlewares>> =
                            route.middlewares.clone().map(|names| {
                                names
                                    .into_iter()
                                    .map(|m| IngressRouteRoutesMiddlewares {
                                        name: format!("{}-{}", &coredb_name, m),
                                        namespace: Some(namespace.to_owned()),
                                    })
                                    .collect()
                            });
                        let route = IngressRouteRoutes {
                            kind: IngressRouteRoutesKind::Rule,
                            r#match: matcher.clone(),
                            services: Some(vec![IngressRouteRoutesServices {
                                name: resource_name.to_string(),
                                port: Some(IntOrString::Int(route.port as i32)),
                                // namespace attribute is NOT a kubernetes namespace
                                // it is the Traefik provider namespace: https://doc.traefik.io/traefik/v3.0/providers/overview/#provider-namespace
                                // https://doc.traefik.io/traefik/v3.0/routing/providers/kubernetes-crd/#kind-middleware
                                namespace: None,
                                kind: Some(IngressRouteRoutesServicesKind::Service),
                                ..IngressRouteRoutesServices::default()
                            }]),
                            middlewares,
                            priority: None,
                        };
                        routes.push(route);
                    }
                    None => {
                        // do not create ingress when there is no path provided
                        continue;
                    }
                }
            }
            Some(routes)
        }
        None => None,
    }
}

pub fn generate_ingress_tcp_routes(
    appsvc: &AppService,
    resource_name: &str,
    namespace: &str,
    host_matcher: String,
    coredb_name: &str,
) -> Option<Vec<IngressRouteTCPRoutes>> {
    match appsvc.routing.clone() {
        Some(routings) => {
            let mut routes: Vec<IngressRouteTCPRoutes> = Vec::new();
            for route in routings.iter() {
                match route.ingress_path.clone() {
                    Some(path) => {
                        if !route.entry_points.clone()?.contains(&"ferretdb".to_string()) {
                            // Do not create IngressRouteTCPRoutes for non-ferretdb routes.
                            continue;
                        }
                        let matcher = format!("{host_matcher} && PathPrefix(`{}`)", path);
                        let middlewares: Option<Vec<IngressRouteTCPRoutesMiddlewares>> =
                            route.middlewares.clone().map(|names| {
                                names
                                    .into_iter()
                                    .map(|m| IngressRouteTCPRoutesMiddlewares {
                                        name: format!("{}-{}", &coredb_name, m),
                                        namespace: Some(namespace.to_owned()),
                                    })
                                    .collect()
                            });
                        let route = IngressRouteTCPRoutes {
                            r#match: matcher.clone(),
                            services: Some(vec![IngressRouteTCPRoutesServices {
                                name: resource_name.to_string(),
                                port: IntOrString::Int(route.port as i32),
                                // namespace attribute is NOT a kubernetes namespace
                                // it is the Traefik provider namespace: https://doc.traefik.io/traefik/v3.0/providers/overview/#provider-namespace
                                // https://doc.traefik.io/traefik/v3.0/routing/providers/kubernetes-crd/#kind-middleware
                                namespace: None,
                                ..IngressRouteTCPRoutesServices::default()
                            }]),
                            middlewares,
                            priority: None,
                        };
                        routes.push(route);
                    }
                    None => {
                        // do not create ingress when there is no path provided
                        continue;
                    }
                }
            }
            Some(routes)
        }
        None => None,
    }
}

pub async fn reconcile_ingress(
    client: Client,
    coredb_name: &str,
    ns: &str,
    oref: OwnerReference,
    desired_routes: Vec<IngressRouteRoutes>,
    desired_middlewares: Vec<Middleware>,
) -> Result<(), kube::Error> {
    let ingress_api: Api<IngressRoute> = Api::namespaced(client.clone(), ns);

    let middleware_api: Api<TraefikMiddleware> = Api::namespaced(client.clone(), ns);
    let desired_middlewares = generate_middlewares(coredb_name, ns, oref.clone(), desired_middlewares);
    let actual_mw_names = get_middlewares(client.clone(), ns, coredb_name).await?;
    let desired_mw_names = desired_middlewares
        .iter()
        .map(|mw| mw.name.clone())
        .collect::<Vec<String>>();
    if let Some(to_delete) = to_delete(desired_mw_names, actual_mw_names) {
        for d in to_delete {
            match middleware_api.delete(&d, &Default::default()).await {
                Ok(_) => {
                    debug!("ns: {}, successfully deleted Middleware: {}", ns, d);
                }
                Err(e) => {
                    error!("ns: {}, Failed to delete Middleware: {}, error: {}", ns, d, e);
                }
            }
        }
    }
    for desired_mw in desired_middlewares {
        match apply_middleware(middleware_api.clone(), &desired_mw.name, &desired_mw.mw).await {
            Ok(_) => {
                debug!("ns: {}, successfully applied Middleware: {}", ns, desired_mw.name);
            }
            Err(e) => {
                error!(
                    "ns: {}, Failed to apply Middleware: {}, error: {}",
                    ns, desired_mw.name, e
                );
            }
        }
    }


    let ingress = generate_ingress(coredb_name, ns, oref, desired_routes.clone());
    if desired_routes.is_empty() {
        // we don't need an IngressRoute when there are no routes
        match ingress_api.get_opt(coredb_name).await {
            Ok(Some(_)) => {
                debug!("Deleting IngressRoute {}.{}", ns, coredb_name);
                ingress_api.delete(coredb_name, &Default::default()).await?;
                return Ok(());
            }
            Ok(None) => {
                warn!("No IngressRoute {}.{} found to delete", ns, coredb_name);
                return Ok(());
            }
            Err(e) => {
                error!(
                    "Error retrieving IngressRoute, {}.{}, error: {}",
                    ns, coredb_name, e
                );
                return Err(e);
            }
        }
    }
    match apply_ingress_route(ingress_api, coredb_name, &ingress).await {
        Ok(_) => {
            debug!("Updated/applied IngressRoute for {}.{}", ns, coredb_name,);
            Ok(())
        }
        Err(e) => {
            error!(
                "Failed to update/apply IngressRoute {}.{}: {}",
                ns, coredb_name, e
            );
            Err(e)
        }
    }
}

pub async fn reconcile_ingress_tcp(
    client: Client,
    coredb_name: &str,
    ns: &str,
    oref: OwnerReference,
    desired_routes: Vec<IngressRouteTCPRoutes>,
    desired_middlewares: Vec<Middleware>,
    entry_points_tcp: Vec<String>,
) -> Result<(), kube::Error> {
    let ingress_api: Api<IngressRouteTCP> = Api::namespaced(client.clone(), ns);

    let middleware_api: Api<TraefikMiddleware> = Api::namespaced(client.clone(), ns);
    let desired_middlewares = generate_middlewares(coredb_name, ns, oref.clone(), desired_middlewares);
    let actual_mw_names = get_middlewares(client.clone(), ns, coredb_name).await?;
    let desired_mw_names = desired_middlewares
        .iter()
        .map(|mw| mw.name.clone())
        .collect::<Vec<String>>();
    if let Some(to_delete) = to_delete(desired_mw_names, actual_mw_names) {
        for d in to_delete {
            match middleware_api.delete(&d, &Default::default()).await {
                Ok(_) => {
                    debug!("ns: {}, successfully deleted Middleware: {}", ns, d);
                }
                Err(e) => {
                    error!("ns: {}, Failed to delete Middleware: {}, error: {}", ns, d, e);
                }
            }
        }
    }
    for desired_mw in desired_middlewares {
        match apply_middleware(middleware_api.clone(), &desired_mw.name, &desired_mw.mw).await {
            Ok(_) => {
                debug!("ns: {}, successfully applied Middleware: {}", ns, desired_mw.name);
            }
            Err(e) => {
                error!(
                    "ns: {}, Failed to apply Middleware: {}, error: {}",
                    ns, desired_mw.name, e
                );
            }
        }
    }


    let ingress = generate_ingress_tcp(coredb_name, ns, oref, desired_routes.clone(), entry_points_tcp);
    if desired_routes.is_empty() {
        // we don't need an IngressRouteTCP when there are no routes
        match ingress_api.get_opt(coredb_name).await {
            Ok(Some(_)) => {
                debug!("Deleting IngressRouteTCP {}.{}", ns, coredb_name);
                ingress_api.delete(coredb_name, &Default::default()).await?;
                return Ok(());
            }
            Ok(None) => {
                warn!("No IngressRouteTCP {}.{} found to delete", ns, coredb_name);
                return Ok(());
            }
            Err(e) => {
                error!(
                    "Error retrieving IngressRouteTCP, {}.{}, error: {}",
                    ns, coredb_name, e
                );
                return Err(e);
            }
        }
    }
    match apply_ingress_route_tcp(ingress_api, coredb_name, &ingress).await {
        Ok(_) => {
            debug!("Updated/applied IngressRouteTCP for {}.{}", ns, coredb_name,);
            Ok(())
        }
        Err(e) => {
            error!(
                "Failed to update/apply IngressRouteTCP {}.{}: {}",
                ns, coredb_name, e
            );
            Err(e)
        }
    }
}

async fn apply_middleware(
    mw_api: Api<TraefikMiddleware>,
    mw_name: &str,
    mw: &TraefikMiddleware,
) -> Result<TraefikMiddleware, kube::Error> {
    let patch_parameters = PatchParams::apply("cntrlr").force();
    mw_api.patch(mw_name, &patch_parameters, &Patch::Apply(&mw)).await
}

async fn apply_ingress_route(
    ingress_api: Api<IngressRoute>,
    ingress_name: &str,
    ingress_route: &IngressRoute,
) -> Result<IngressRoute, kube::Error> {
    let patch_parameters = PatchParams::apply("cntrlr").force();
    ingress_api
        .patch(ingress_name, &patch_parameters, &Patch::Apply(&ingress_route))
        .await
}

async fn apply_ingress_route_tcp(
    ingress_api: Api<IngressRouteTCP>,
    ingress_name: &str,
    ingress_route_tcp: &IngressRouteTCP,
) -> Result<IngressRouteTCP, kube::Error> {
    let patch_parameters = PatchParams::apply("cntrlr").force();
    ingress_api
        .patch(ingress_name, &patch_parameters, &Patch::Apply(&ingress_route_tcp))
        .await
}

async fn get_middlewares(
    client: Client,
    namespace: &str,
    coredb_name: &str,
) -> Result<Vec<String>, kube::Error> {
    let label_selector = format!("component={},coredb.io/name={}", COMPONENT_NAME, coredb_name);
    let deployent_api: Api<TraefikMiddleware> = Api::namespaced(client, namespace);
    let lp = ListParams::default().labels(&label_selector).timeout(10);
    let deployments = deployent_api.list(&lp).await?;
    Ok(deployments
        .items
        .iter()
        .map(|d| d.metadata.name.to_owned().expect("no name on resource"))
        .collect())
}
