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

use crate::{
    apis::coredb_types::CoreDB,
    app_service::types::COMPONENT_NAME,
    errors::OperatorError,
    traefik::middleware_tcp_crd::{MiddlewareTCP, MiddlewareTCPIpAllowList, MiddlewareTCPSpec},
    Context,
};
use tracing::{debug, error, info, warn};

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

async fn delete_ingress_route_tcp(
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

// 1) We should never delete or update the hostname of an ingress route tcp.
//    Instead, just create another one if the hostname does not match.
//    This allows for domain name reconfiguration (e.g. coredb.io -> tembo.io),
//    with the old connection string still working.
//
// 2) We should replace the service and port target of all ingress route tcp
//    During a migration, the Service target will change, for example from CoreDB-operator managed
//    to CNPG managed read-write endpoints.
//
// 3) We should allow for additional ingress route tcp to be created for different use cases
//    For example read-only endpoints, we should not accidentally handle these other
//    IngressRouteTCP in this code, so we check that we are working with the correct type of Service.
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
) -> Result<(), OperatorError> {
    let client = ctx.client.clone();
    // Initialize kube api for ingress route tcp
    let ingress_route_tcp_api: Api<IngressRouteTCP> = Api::namespaced(client, namespace);
    let owner_reference = cdb.controller_owner_ref(&()).unwrap();

    // get all IngressRouteTCPs in the namespace
    // After CNPG migration is done, this can look for only ingress route tcp with the correct owner reference
    let ingress_route_tcps = ingress_route_tcp_api.list(&Default::default()).await?;

    // We will save information about the existing ingress route tcp(s) in these vectors
    let mut present_matchers_list: Vec<String> = vec![];
    let mut present_ing_route_tcp_names_list: Vec<String> = vec![];

    // Check for all existing IngressRouteTCPs in this namespace
    // Filter out any that are not for this DB, do not have the correct prefix, or do not have matching service name
    for ingress_route_tcp in &ingress_route_tcps {
        // Get labels for this ingress route tcp
        let labels = ingress_route_tcp
            .metadata
            .labels
            .clone()
            .unwrap_or_default();
        // Check whether labels includes component=appService
        let app_svc_label = labels
            .get("component")
            .map(|component| component == COMPONENT_NAME)
            .unwrap_or(false);

        let ingress_route_tcp_name = match ingress_route_tcp.metadata.name.clone() {
            Some(ingress_route_tcp_name) => {
                if app_svc_label {
                    debug!(
                        "Skipping ingress route tcp with appService label: {}",
                        ingress_route_tcp_name
                    );
                    continue;
                }

                if !(ingress_route_tcp_name.starts_with(ingress_name_prefix)
                    || ingress_route_tcp_name == cdb.name_any())
                {
                    debug!(
                        "Skipping ingress route tcp without prefix {}: {}",
                        ingress_name_prefix, ingress_route_tcp_name
                    );
                    continue;
                }
                ingress_route_tcp_name
            }
            None => {
                error!(
                    "IngressRouteTCP {}.{}, does not have a name.",
                    subdomain, basedomain
                );
                return Err(OperatorError::IngressRouteTCPName);
            }
        };
        debug!(
            "Detected ingress route tcp endpoint {}.{}",
            ingress_route_tcp_name, namespace
        );

        // Get the settings of our ingress route tcp, so we can update to a new
        // endpoint, if needed.

        let service_name_actual = ingress_route_tcp.spec.routes[0]
            .services
            .as_ref()
            .expect("Ingress route has no services")[0]
            .name
            .clone();

        if service_name_actual.as_str() != service_name {
            continue;
        }

        let service_port_actual = ingress_route_tcp.spec.routes[0]
            .services
            .as_ref()
            .expect("Ingress route has no services")[0]
            .port
            .clone();

        // Save list of names so we can pick a name that doesn't exist,
        // if we need to create a new ingress route tcp.
        present_ing_route_tcp_names_list.push(ingress_route_tcp_name.clone());

        // Keep the existing matcher (domain name) when updating an existing IngressRouteTCP,
        // so that we do not break connection strings with domain name updates.
        let matcher_actual = ingress_route_tcp.spec.routes[0].r#match.clone();

        // Save the matchers to know if we need to create a new ingress route tcp or not.
        present_matchers_list.push(matcher_actual.clone());

        // Check if either the service name or port are mismatched
        if !(app_svc_label || service_name_actual == service_name && service_port_actual == port) {
            // This situation should only occur when the service name or port is changed, for example during cut-over from
            // CoreDB operator managing the service to CNPG managing the service.
            warn!(
                "Postgres IngressRouteTCP {}.{}, does not match the service name or port. Updating service or port and leaving the match rule the same.",
                ingress_route_tcp_name, namespace
            );

            // We will keep the matcher and the name the same, but update the service name and port.
            // Also, we will set ownership.
            let ingress_route_tcp_to_apply = postgres_ingress_route_tcp(
                ingress_route_tcp_name.clone(),
                namespace.to_string(),
                owner_reference.clone(),
                matcher_actual,
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
        }
    }

    // At this point in the code, all applicable IngressRouteTCPs are pointing to the right
    // service and port. Now, we just need to create a new IngressRouteTCP if we do not already
    // have one for the specified domain name.

    // Build the expected IngressRouteTCP matcher we expect to find
    let newest_matcher = format!("HostSNI(`{subdomain}.{basedomain}`)");

    if !present_matchers_list.contains(&newest_matcher) {
        // In this block, we are creating a new IngressRouteTCP

        // Pick a name for a new ingress route tcp that doesn't already exist
        let mut index = 0;
        let mut ingress_route_tcp_name_new = format!("{}{}", ingress_name_prefix, index);
        while present_ing_route_tcp_names_list.contains(&ingress_route_tcp_name_new) {
            index += 1;
            ingress_route_tcp_name_new = format!("{}{}", ingress_name_prefix, index);
        }
        let ingress_route_tcp_name_new = ingress_route_tcp_name_new;

        let ingress_route_tcp_to_apply = postgres_ingress_route_tcp(
            ingress_route_tcp_name_new.clone(),
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
            &ingress_route_tcp_name_new,
            &ingress_route_tcp_to_apply,
        )
        .await?;
    } else {
        debug!(
            "There is already an IngressRouteTCP for this matcher, so we don't need to create a new one: {}",
            newest_matcher
        );
    }

    // Check that all the existing IngressRouteTCPs include the middleware(s),
    // and add them if they don't.
    let mut middlewares_to_add = Vec::new();
    for middleware_name in middleware_names.iter() {
        middlewares_to_add.push(IngressRouteTCPRoutesMiddlewares {
            name: middleware_name.clone(),
            // Warning: 'namespace' field does not mean kubernetes namespace,
            // it means Traefik 'provider' namespace.
            // The IngressRouteTCP will by default look in the same Kubernetes namespace,
            // so this should be set to None.
            // https://doc.traefik.io/traefik/providers/overview/#provider-namespace
            namespace: None,
        });
    }
    let middlewares_to_add = match middlewares_to_add.len() {
        0 => None,
        _ => Some(middlewares_to_add),
    };

    for ingress_route_tcp in ingress_route_tcps {
        // Get labels for this ingress route tcp
        let labels = ingress_route_tcp
            .metadata
            .labels
            .clone()
            .unwrap_or_default();
        // Check whether labels includes component=appService
        let app_svc_label = labels
            .get("component")
            .map(|component| component == COMPONENT_NAME)
            .unwrap_or(false);
        let ingress_route_tcp_name = ingress_route_tcp.metadata.name.clone().unwrap();
        if present_ing_route_tcp_names_list.contains(&ingress_route_tcp_name) {
            // Skip any ingress route tcp that is not matched for this database
            continue;
        }
        // Check if the middleware is already included
        let mut needs_middleware_update = false;
        for route in &ingress_route_tcp.spec.routes {
            if route.middlewares != middlewares_to_add {
                needs_middleware_update = true;
                break;
            }
        }
        if needs_middleware_update && !app_svc_label {
            info!(
                "Adding middleware to existing IngressRouteTCP {} for db {}",
                &ingress_route_tcp_name,
                cdb.metadata.name.clone().unwrap()
            );
            // We need to add the middleware to this ingress route tcp
            let mut ingress_route_tcp_to_apply = ingress_route_tcp.clone();
            for route in &mut ingress_route_tcp_to_apply.spec.routes {
                route.middlewares = middlewares_to_add.clone();
            }

            // Apply this ingress route tcp
            apply_ingress_route_tcp(
                ingress_route_tcp_api.clone(),
                namespace,
                &ingress_route_tcp_name,
                &ingress_route_tcp_to_apply,
            )
            .await?;
        }
    }

    Ok(())
}

fn generate_ip_allow_list_middleware_tcp(cdb: &CoreDB) -> MiddlewareTCP {
    let source_range = match cdb.spec.ip_allow_list.clone() {
        None => {
            vec![]
        }
        Some(ips) => ips,
    };

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
