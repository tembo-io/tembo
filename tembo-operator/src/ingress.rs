use crate::ingress_route_tcp_crd::{
    IngressRouteTCP, IngressRouteTCPRoutes, IngressRouteTCPRoutesServices, IngressRouteTCPSpec,
    IngressRouteTCPTls,
};
use k8s_openapi::apimachinery::pkg::{
    apis::meta::v1::{ObjectMeta, OwnerReference},
    util::intstr::IntOrString,
};
use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use std::sync::Arc;

use crate::{apis::coredb_types::CoreDB, errors::OperatorError, Context};
use tracing::{debug, error, info, warn};

fn postgres_ingress_route_tcp(
    name: String,
    namespace: String,
    owner_reference: OwnerReference,
    matcher: String,
    service_name: String,
    port: IntOrString,
) -> IngressRouteTCP {
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
                    namespace: None,
                    proxy_protocol: None,
                    termination_delay: None,
                    weight: None,
                }]),
                middlewares: None,
                priority: None,
            }],
            tls: Some(IngressRouteTCPTls {
                passthrough: Some(true),
                cert_resolver: None,
                domains: None,
                options: None,
                secret_name: None,
                store: None,
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
        owner_reference.clone(),
        matcher_actual.clone(),
        service_name_read_write.to_string(),
        port.clone(),
    );
    let ingress_route_tcp_api: Api<IngressRouteTCP> = Api::namespaced(ctx.client.clone(), namespace);
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

async fn apply_ingress_route_tcp(
    ingress_route_tcp_api: Api<IngressRouteTCP>,
    namespace: &str,
    ingress_route_tcp_name: &String,
    ingress_route_tcp_to_apply: &IngressRouteTCP,
) -> Result<(), OperatorError> {
    let patch = Patch::Apply(&ingress_route_tcp_to_apply);
    let patch_parameters = PatchParams::apply("cntrlr").force();
    match ingress_route_tcp_api
        .patch(&ingress_route_tcp_name.clone(), &patch_parameters, &patch)
        .await
    {
        Ok(_) => {
            info!(
                "Updated postgres read and write IngressRouteTCP {}.{}",
                ingress_route_tcp_name.clone(),
                namespace
            );
        }
        Err(e) => {
            error!(
                "Failed to update postgres read and write IngressRouteTCP {}.{}: {}",
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
    service_name_read_write: &str,
    port: IntOrString,
) -> Result<(), OperatorError> {
    let client = ctx.client.clone();
    // Initialize kube api for ingress route tcp
    let ingress_route_tcp_api: Api<IngressRouteTCP> = Api::namespaced(client, namespace);
    let owner_reference = cdb.controller_owner_ref(&()).unwrap();

    // get all IngressRouteTCPs in the namespace
    // After CNPG migration is done, this can look for only ingress route tcp with the correct owner reference
    let ingress_route_tcps = ingress_route_tcp_api.list(&Default::default()).await?;

    // Prefix by resource name allows multiple per namespace
    let ingress_route_tcp_name_prefix_rw = format!("{}-rw-", cdb.name_any());
    let ingress_route_tcp_name_prefix_rw = ingress_route_tcp_name_prefix_rw.as_str();

    // We will save information about the existing ingress route tcp(s) in these vectors
    let mut present_matchers_list: Vec<String> = vec![];
    let mut present_ing_route_tcp_names_list: Vec<String> = vec![];

    // Check for all existing IngressRouteTCPs in this namespace
    // Filter out any that are not for this DB or are not read-write
    for ingress_route_tcp in ingress_route_tcps {
        let ingress_route_tcp_name = match ingress_route_tcp.metadata.name.clone() {
            Some(ingress_route_tcp_name) => {
                if !(ingress_route_tcp_name.starts_with(ingress_route_tcp_name_prefix_rw)
                    || ingress_route_tcp_name == cdb.name_any())
                {
                    debug!(
                        "Skipping non postgres-rw ingress route tcp: {}",
                        ingress_route_tcp_name
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
            "Detected ingress route tcp read write endpoint {}.{}",
            ingress_route_tcp_name, namespace
        );
        // Save list of names so we can pick a name that doesn't exist,
        // if we need to create a new ingress route tcp.
        present_ing_route_tcp_names_list.push(ingress_route_tcp_name.clone());

        // Get the settings of our ingress route tcp, so we can update to a new
        // endpoint, if needed.

        let service_name_actual = ingress_route_tcp.spec.routes[0]
            .services
            .clone()
            .expect("Ingress route has no services")[0]
            .name
            .clone();
        let service_port_actual = ingress_route_tcp.spec.routes[0]
            .services
            .clone()
            .expect("Ingress route has no services")[0]
            .port
            .clone();

        // Keep the existing matcher (domain name) when updating an existing IngressRouteTCP,
        // so that we do not break connection strings with domain name updates.
        let matcher_actual = ingress_route_tcp.spec.routes[0].r#match.clone();

        // Save the matchers to know if we need to create a new ingress route tcp or not.
        present_matchers_list.push(matcher_actual.clone());

        // Check if either the service name or port are mismatched
        if !(service_name_actual == service_name_read_write && service_port_actual == port) {
            // This situation should only occur when the service name or port is changed, for example during cut-over from
            // CoreDB operator managing the service to CNPG managing the service.
            warn!(
                "Postgres read and write IngressRouteTCP {}.{}, does not match the service name or port. Updating service or port and leaving the match rule the same.",
                ingress_route_tcp_name, namespace
            );

            // We will keep the matcher and the name the same, but update the service name and port.
            // Also, we will set ownership.
            let ingress_route_tcp_to_apply = postgres_ingress_route_tcp(
                ingress_route_tcp_name.clone(),
                namespace.to_string(),
                owner_reference.clone(),
                matcher_actual.clone(),
                service_name_read_write.to_string(),
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
        let mut ingress_route_tcp_name_new = format!("{}{}", ingress_route_tcp_name_prefix_rw, index);
        while present_ing_route_tcp_names_list.contains(&ingress_route_tcp_name_new) {
            index += 1;
            ingress_route_tcp_name_new = format!("{}{}", ingress_route_tcp_name_prefix_rw, index);
        }
        let ingress_route_tcp_name_new = ingress_route_tcp_name_new;

        let ingress_route_tcp_to_apply = postgres_ingress_route_tcp(
            ingress_route_tcp_name_new.clone(),
            namespace.to_string(),
            owner_reference.clone(),
            newest_matcher.clone(),
            service_name_read_write.to_string(),
            port.clone(),
        );
        // Apply this ingress route tcp
        apply_ingress_route_tcp(
            ingress_route_tcp_api,
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

    Ok(())
}
