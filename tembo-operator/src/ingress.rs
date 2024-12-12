use crate::traefik::ingress_route_tcp_crd::{
    IngressRouteTCP, IngressRouteTCPRoutes, IngressRouteTCPRoutesMiddlewares,
    IngressRouteTCPRoutesServices, IngressRouteTCPSpec, IngressRouteTCPTls,
};
use k8s_openapi::apimachinery::pkg::{
    apis::meta::v1::{ObjectMeta, OwnerReference},
    util::intstr::IntOrString,
};
use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use regex::Regex;
use std::sync::Arc;

use crate::ingress_route_crd::IngressRoute;
use crate::{
    apis::coredb_types::CoreDB,
    errors::OperatorError,
    traefik::middleware_tcp_crd::{MiddlewareTCP, MiddlewareTCPIpAllowList, MiddlewareTCPSpec},
    Context,
};
use tracing::{debug, error, info};

pub const VALID_IPV4_CIDR_BLOCK: &str = "^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)(/(3[0-2]|2[0-9]|1[0-9]|[0-9]))?$";

fn postgres_ingress_route_tcp(
    name: String,
    namespace: String,
    owner_reference: OwnerReference,
    matcher: String,
    service_name: String,
    middleware_names: Vec<String>,
    port: IntOrString,
) -> IngressRouteTCP {
    let mut middlewares = vec![];
    for middleware_name in middleware_names {
        middlewares.push(IngressRouteTCPRoutesMiddlewares {
            name: middleware_name.clone(),
            // Warning: 'namespace' field does not mean kubernetes namespace,
            // it means Traefik 'provider' namespace.
            // The IngressRouteTCP will by default look in the same Kubernetes namespace,
            // so this should be set to None.
            // https://doc.traefik.io/traefik/providers/overview/#provider-namespace
            namespace: None,
        });
    }
    let middlewares = Some(middlewares);

    IngressRouteTCP {
        metadata: ObjectMeta {
            name: Some(name),
            namespace: Some(namespace),
            owner_references: Some(vec![owner_reference]),
            ..ObjectMeta::default()
        },
        spec: IngressRouteTCPSpec {
            entry_points: Some(vec!["postgresql".to_string()]),
            routes: vec![IngressRouteTCPRoutes {
                r#match: matcher,
                services: Some(vec![IngressRouteTCPRoutesServices {
                    name: service_name,
                    port,
                    ..IngressRouteTCPRoutesServices::default()
                }]),
                middlewares,
                priority: None,
                syntax: None,
            }],
            tls: Some(IngressRouteTCPTls {
                passthrough: Some(true),
                ..IngressRouteTCPTls::default()
            }),
        },
    }
}

// For end-user provided, extra domain names,
// we allow for update and deletion of domain names.
pub async fn reconcile_extra_postgres_ing_route_tcp(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    namespace: &str,
    service_name_read_write: &str,
    port: IntOrString,
    middleware_names: Vec<String>,
) -> Result<(), OperatorError> {
    let mut extra_domain_names = cdb.spec.extra_domains_rw.clone().unwrap_or_default();
    // Ensure always same order
    extra_domain_names.sort();
    let matchers = extra_domain_names
        .iter()
        .map(|domain_name| format!("HostSNI(`{}`)", domain_name))
        .collect::<Vec<String>>();
    let matcher_actual = matchers.join(" || ");
    let ingress_route_tcp_name = format!("extra-{}-rw", cdb.name_any());
    let owner_reference = cdb.controller_owner_ref(&()).unwrap();

    let ingress_route_tcp_to_apply = postgres_ingress_route_tcp(
        ingress_route_tcp_name.clone(),
        namespace.to_string(),
        owner_reference,
        matcher_actual,
        service_name_read_write.to_string(),
        middleware_names,
        port,
    );
    let ingress_route_tcp_api: Api<IngressRouteTCP> =
        Api::namespaced(ctx.client.clone(), namespace);
    if !extra_domain_names.is_empty() {
        apply_ingress_route_tcp(
            ingress_route_tcp_api,
            namespace,
            &ingress_route_tcp_name,
            &ingress_route_tcp_to_apply,
        )
        .await
    } else {
        delete_ingress_route_tcp(ingress_route_tcp_api, namespace, &ingress_route_tcp_name).await
    }
}

// For end-user provided, extra domain names,
// we allow for update and deletion of domain names.
pub async fn reconcile_ip_allowlist_middleware(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<String, OperatorError> {
    let ip_allow_list_middleware = generate_ip_allow_list_middleware_tcp(cdb);
    let middleware_name = ip_allow_list_middleware
        .metadata
        .name
        .clone()
        .expect("middleware is always named");
    let namespace = &ip_allow_list_middleware
        .metadata
        .namespace
        .clone()
        .expect("namespace is always set");

    let middleware_api: Api<MiddlewareTCP> = Api::namespaced(ctx.client.clone(), namespace);

    let patch = Patch::Apply(&ip_allow_list_middleware);
    let patch_parameters = PatchParams::apply("cntrlr").force();
    match middleware_api
        .patch(&middleware_name.clone(), &patch_parameters, &patch)
        .await
    {
        Ok(_) => {
            debug!(
                "Updated MiddlewareTCP {}.{}",
                middleware_name.clone(),
                &namespace
            );
        }
        Err(e) => {
            // serialize then log json of middlewaretcp
            let serialized = serde_json::to_string(&ip_allow_list_middleware).unwrap_or_default();
            error!(
                "Failed to update MiddlewareTCP {}.{}: {} \n {}",
                middleware_name, namespace, e, serialized
            );
            return Err(OperatorError::IngressRouteTcpError);
        }
    }

    Ok(middleware_name)
}

async fn apply_ingress_route_tcp(
    ingress_route_tcp_api: Api<IngressRouteTCP>,
    namespace: &str,
    ingress_route_tcp_name: &String,
    ingress_route_tcp_to_apply: &IngressRouteTCP,
) -> Result<(), OperatorError> {
    let patch: Patch<&&IngressRouteTCP> = Patch::Apply(&ingress_route_tcp_to_apply);
    let patch_parameters = PatchParams::apply("cntrlr").force();
    match ingress_route_tcp_api
        .patch(&ingress_route_tcp_name.clone(), &patch_parameters, &patch)
        .await
    {
        Ok(_) => {
            info!(
                "Updated postgres IngressRouteTCP {}.{}",
                ingress_route_tcp_name.clone(),
                namespace
            );
        }
        Err(e) => {
            error!(
                "Failed to update postgres IngressRouteTCP {}.{}: {}",
                ingress_route_tcp_name, namespace, e
            );
            return Err(OperatorError::IngressRouteTcpError);
        }
    }
    Ok(())
}

pub async fn delete_ingress_route(
    ingress_route_api: Api<IngressRoute>,
    namespace: &str,
    ingress_route_name: &String,
) -> Result<(), OperatorError> {
    // Check if the resource exists
    if ingress_route_api
        .get(&ingress_route_name.clone())
        .await
        .is_ok()
    {
        // If it exists, proceed with the deletion
        let delete_parameters = DeleteParams::default();
        match ingress_route_api
            .delete(&ingress_route_name.clone(), &delete_parameters)
            .await
        {
            Ok(_) => {
                info!(
                    "Deleted IngressRoute {}.{}",
                    ingress_route_name.clone(),
                    namespace
                );
            }
            Err(e) => {
                error!(
                    "Failed to delete IngressRoute {}.{}: {}",
                    ingress_route_name, namespace, e
                );
                return Err(OperatorError::IngressRouteError);
            }
        }
    } else {
        debug!(
            "IngressRoute {}.{} was not found. Assuming it's already deleted.",
            ingress_route_name, namespace
        );
    }
    Ok(())
}

pub async fn delete_ingress_route_tcp(
    ingress_route_tcp_api: Api<IngressRouteTCP>,
    namespace: &str,
    ingress_route_tcp_name: &String,
) -> Result<(), OperatorError> {
    // Check if the resource exists
    if ingress_route_tcp_api
        .get(&ingress_route_tcp_name.clone())
        .await
        .is_ok()
    {
        // If it exists, proceed with the deletion
        let delete_parameters = DeleteParams::default();
        match ingress_route_tcp_api
            .delete(&ingress_route_tcp_name.clone(), &delete_parameters)
            .await
        {
            Ok(_) => {
                info!(
                    "Deleted IngressRouteTCP {}.{}",
                    ingress_route_tcp_name.clone(),
                    namespace
                );
            }
            Err(e) => {
                error!(
                    "Failed to delete IngressRouteTCP {}.{}: {}",
                    ingress_route_tcp_name, namespace, e
                );
                return Err(OperatorError::IngressRouteTcpError);
            }
        }
    } else {
        debug!(
            "IngressRouteTCP {}.{} was not found. Assuming it's already deleted.",
            ingress_route_tcp_name, namespace
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn reconcile_postgres_ing_route_tcp(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    subdomain: &str,
    basedomain: &str,
    namespace: &str,
    ingress_name_prefix: &str,
    service_name: &str,
    port: IntOrString,
    middleware_names: Vec<String>,
    delete: bool,
) -> Result<(), OperatorError> {
    let client = ctx.client.clone();
    // Initialize kube api for ingress route tcp
    let ingress_route_tcp_api: Api<IngressRouteTCP> = Api::namespaced(client, namespace);
    let owner_reference = cdb.controller_owner_ref(&()).unwrap();
    let ingress_route_tcp_name = format!("{}0", ingress_name_prefix);
    let newest_matcher = format!("HostSNI(`{subdomain}.{basedomain}`)");
    if delete {
        delete_ingress_route_tcp(
            ingress_route_tcp_api.clone(),
            namespace,
            &ingress_route_tcp_name,
        )
        .await?;
        return Ok(());
    }

    let ingress_route_tcp_to_apply = postgres_ingress_route_tcp(
        ingress_route_tcp_name.clone(),
        namespace.to_string(),
        owner_reference.clone(),
        newest_matcher.clone(),
        service_name.to_string(),
        middleware_names.clone(),
        port.clone(),
    );

    // Apply this ingress route tcp
    apply_ingress_route_tcp(
        ingress_route_tcp_api.clone(),
        namespace,
        &ingress_route_tcp_name,
        &ingress_route_tcp_to_apply,
    )
    .await?;

    Ok(())
}

fn generate_ip_allow_list_middleware_tcp(cdb: &CoreDB) -> MiddlewareTCP {
    let source_range = cdb.spec.ip_allow_list.clone().unwrap_or_default();

    let mut valid_ips = valid_cidrs(&source_range);

    for ip in &source_range {
        if !valid_ips.contains(ip) {
            error!(
                "Invalid IP address or CIDR block '{}' on DB {}, skipping",
                ip,
                cdb.name_any()
            );
        }
    }

    if valid_ips.is_empty() {
        // If IP allow list is not specified, allow all IPs
        debug!(
            "No valid IP addresses or CIDR blocks specified for DB {}, allowing all IPs",
            cdb.name_any()
        );
        valid_ips.push("0.0.0.0/0".to_string());
    }

    let owner_references = cdb.controller_owner_ref(&()).map(|oref| vec![oref]);

    MiddlewareTCP {
        metadata: ObjectMeta {
            name: Some(cdb.name_any()),
            namespace: cdb.namespace(),
            owner_references,
            ..Default::default()
        },
        spec: MiddlewareTCPSpec {
            ip_allow_list: Some(MiddlewareTCPIpAllowList {
                source_range: Some(valid_ips),
            }),
            ..Default::default()
        },
    }
}

pub fn valid_cidrs(source_range: &[String]) -> Vec<String> {
    // Validate each IP address or CIDR block against the regex
    let cidr_regex =
        Regex::new(VALID_IPV4_CIDR_BLOCK).expect("Failed to compile regex for IPv4 CIDR block");
    let mut valid_ips = Vec::new();
    for ip in source_range.iter() {
        if !cidr_regex.is_match(ip) {
        } else {
            valid_ips.push(ip.clone());
        }
    }
    valid_ips.sort();
    valid_ips
}

#[cfg(test)]
mod tests {
    use crate::{
        apis::coredb_types::{CoreDB, CoreDBSpec},
        ingress::generate_ip_allow_list_middleware_tcp,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    #[test]
    fn test_no_ip_allow_list() {
        let cdb = CoreDB {
            metadata: ObjectMeta::default(),
            spec: CoreDBSpec {
                ip_allow_list: None,
                ..CoreDBSpec::default()
            },
            status: None,
        };
        let result = generate_ip_allow_list_middleware_tcp(&cdb);
        let source_range = result
            .spec
            .ip_allow_list
            .clone()
            .unwrap()
            .source_range
            .unwrap();
        assert_eq!(source_range.len(), 1);
        assert!(
            source_range.contains(&"0.0.0.0/0".to_string()),
            "{:?}",
            source_range
        );
    }

    #[test]
    fn test_invalid_ips() {
        let cdb = CoreDB {
            metadata: ObjectMeta::default(),
            spec: CoreDBSpec {
                ip_allow_list: Some(vec!["10.0.0.256".to_string(), "192.168.1.0/33".to_string()]),
                ..CoreDBSpec::default()
            },
            status: None,
        };
        let result = generate_ip_allow_list_middleware_tcp(&cdb);
        let source_range = result
            .spec
            .ip_allow_list
            .clone()
            .unwrap()
            .source_range
            .unwrap();
        assert_eq!(source_range.len(), 1);
        assert!(
            source_range.contains(&"0.0.0.0/0".to_string()),
            "{:?}",
            source_range
        );
    }

    #[test]
    fn test_mixed_ips() {
        let cdb = CoreDB {
            metadata: ObjectMeta::default(),
            spec: CoreDBSpec {
                ip_allow_list: Some(vec![
                    "10.0.0.1".to_string(),
                    "192.168.1.0/24".to_string(),
                    "10.0.0.255".to_string(),
                ]),
                ..CoreDBSpec::default()
            },
            status: None,
        };
        let result = generate_ip_allow_list_middleware_tcp(&cdb);
        // Add assertions to ensure that the middleware contains the correct IPs and not the invalid one
        let source_range = result
            .spec
            .ip_allow_list
            .clone()
            .unwrap()
            .source_range
            .unwrap();
        assert_eq!(source_range.len(), 3);
        assert!(
            source_range.contains(&"10.0.0.1".to_string()),
            "{:?}",
            source_range
        );
        assert!(
            source_range.contains(&"192.168.1.0/24".to_string()),
            "{:?}",
            source_range
        );
        assert!(
            source_range.contains(&"10.0.0.255".to_string()),
            "{:?}",
            source_range
        );
    }
}
