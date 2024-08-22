use crate::{apis::coredb_types::CoreDB, Context};
use k8s_openapi::api::networking::v1::NetworkPolicy;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt},
    client::Client,
};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::{
    errors::OperatorError,
    ingress::{reconcile_ip_allowlist_middleware, reconcile_postgres_ing_route_tcp},
    network_policies::apply_network_policy,
};
use k8s_openapi::api::core::v1::Service;

/// Reconcile dedicated networking resources for the CoreDB instance.
///
/// This function handles the creation, update, or deletion of Kubernetes resources
/// required for dedicated networking, including services, network policies, and
/// ingress routes.
///
/// # Parameters
/// - `cdb`: The CoreDB custom resource instance.
/// - `ctx`: The operator context containing the Kubernetes client and other configurations.
pub async fn reconcile_dedicated_networking(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<(), OperatorError> {
    let ns = cdb.namespace().unwrap_or_else(|| {
        error!(
            "Namespace not found for CoreDB instance: {}",
            cdb.name_any()
        );
        "default".to_string()
    });
    let basedomain = env::var("DATA_PLANE_BASEDOMAIN").unwrap_or_else(|_| "localhost".to_string());
    let port = IntOrString::Int(5432);
    let client = ctx.client.clone();

    info!(
        "Starting reconciliation of dedicated networking for CoreDB instance: {} in namespace: {}",
        cdb.name_any(),
        ns
    );

    if let Some(dedicated_networking) = &cdb.spec.dedicatedNetworking {
        if dedicated_networking.enabled {
            info!(
                "Dedicated networking is enabled for CoreDB instance: {}",
                cdb.name_any()
            );

            // Reconcile Network Policies
            info!(
                "Reconciling network policies for dedicated networking in namespace: {}",
                ns
            );
            reconcile_dedicated_networking_network_policies(
                client.clone(),
                &ns,
                &cdb.name_any(),
                "172.31.0.0/16", // Replace with the actual CIDR for your dedicated network
            )
            .await
            .map_err(|e| {
                error!("Failed to reconcile network policies: {:?}", e);
                e
            })?;

            // Reconcile IP Allowlist Middleware
            info!(
                "Reconciling IP allow list middleware for CoreDB instance: {}",
                cdb.name_any()
            );
            let middleware_name = reconcile_ip_allowlist_middleware(cdb, ctx.clone())
                .await
                .map_err(|e| {
                    error!("Failed to reconcile IP allowlist middleware: {:?}", e);
                    e
                })?;

            // Reconcile Primary Service Ingress
            info!(
                "Handling primary service ingress for CoreDB instance: {}",
                cdb.name_any()
            );
            reconcile_dedicated_networking_service(
                client.clone(),
                &ns,
                &cdb.name_any(),
                dedicated_networking.public,
                false,
            )
            .await
            .map_err(|e| {
                error!("Failed to reconcile primary service ingress: {:?}", e);
                e
            })?;

            reconcile_dedicated_networking_ing_route_tcp(
                cdb,
                ctx.clone(),
                &ns,
                &basedomain,
                "dedicated",
                &format!("{}-dedicated", cdb.name_any()),
                port.clone(),
                vec![middleware_name.clone()],
                false,
                false, // primary service
            )
            .await
            .map_err(|e| {
                error!("Failed to reconcile primary ingress route: {:?}", e);
                e
            })?;

            // Handle Standby Service Ingress if included
            if dedicated_networking.includeStandby {
                info!(
                    "Handling standby service ingress for CoreDB instance: {}",
                    cdb.name_any()
                );
                reconcile_dedicated_networking_service(
                    client.clone(),
                    &ns,
                    &cdb.name_any(),
                    dedicated_networking.public,
                    true,
                )
                .await
                .map_err(|e| {
                    error!("Failed to reconcile standby service ingress: {:?}", e);
                    e
                })?;

                reconcile_dedicated_networking_ing_route_tcp(
                    cdb,
                    ctx.clone(),
                    &ns,
                    &basedomain,
                    "dedicated-ro",
                    &format!("{}-dedicated-ro", cdb.name_any()),
                    port.clone(),
                    vec![middleware_name],
                    false,
                    true,
                )
                .await
                .map_err(|e| {
                    error!("Failed to reconcile standby ingress route: {:?}", e);
                    e
                })?;
            }
        } else {
            // If dedicated networking is disabled, clean up resources
            info!(
                "Dedicated networking is disabled. Deleting services and ingress routes for CoreDB instance: {}",
                cdb.name_any()
            );

            delete_dedicated_networking_service(client.clone(), &ns, &cdb.name_any(), false)
                .await
                .map_err(|e| {
                    error!("Failed to delete primary service: {:?}", e);
                    e
                })?;

            delete_dedicated_networking_service(client.clone(), &ns, &cdb.name_any(), true)
                .await
                .map_err(|e| {
                    error!("Failed to delete standby service: {:?}", e);
                    e
                })?;

            reconcile_dedicated_networking_ing_route_tcp(
                cdb,
                ctx.clone(),
                &ns,
                &basedomain,
                "dedicated",
                &format!("{}-dedicated", cdb.name_any()),
                port.clone(),
                vec![],
                true,
                false,
            )
            .await
            .map_err(|e| {
                error!("Failed to delete primary ingress route: {:?}", e);
                e
            })?;

            reconcile_dedicated_networking_ing_route_tcp(
                cdb,
                ctx,
                &ns,
                &basedomain,
                "dedicated-ro",
                &format!("{}-dedicated-ro", cdb.name_any()),
                port,
                vec![],
                true,
                true,
            )
            .await
            .map_err(|e| {
                error!("Failed to delete standby ingress route: {:?}", e);
                e
            })?;
        }
    } else {
        info!(
            "Dedicated networking is not configured for CoreDB instance: {}",
            cdb.name_any()
        );
    }

    info!(
        "Completed reconciliation of dedicated networking for CoreDB instance: {} in namespace: {}",
        cdb.name_any(),
        ns
    );
    Ok(())
}

/// Reconcile the network policies for dedicated networking.
///
/// This function applies a specific network policy that allows traffic from a specified CIDR block
/// to the CoreDB pods.
///
/// # Parameters
/// - `client`: The Kubernetes client.
/// - `namespace`: The namespace in which to apply the network policy.
/// - `cdb_name`: The name of the CoreDB instance.
/// - `cidr`: The CIDR block to allow traffic from.
async fn reconcile_dedicated_networking_network_policies(
    client: Client,
    namespace: &str,
    cdb_name: &str,
    cidr: &str,
) -> Result<(), OperatorError> {
    let np_api: Api<NetworkPolicy> = Api::namespaced(client, namespace);

    let policy_name = format!("{}-allow-nlb", cdb_name);
    info!(
        "Applying network policy: {} in namespace: {} to allow traffic from CIDR: {}",
        policy_name, namespace, cidr
    );

    let dedicated_network_policy = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": policy_name,
            "namespace": namespace,
        },
        "spec": {
            "podSelector": {
                "matchLabels": {
                    "cnpg.io/cluster": cdb_name
                }
            },
            "policyTypes": ["Ingress"],
            "ingress": [
                {
                    "from": [
                        {
                            "ipBlock": {
                                "cidr": cidr
                            }
                        }
                    ],
                    "ports": [
                        {
                            "protocol": "TCP",
                            "port": 5432
                        }
                    ]
                }
            ]
        }
    });

    apply_network_policy(namespace, &np_api, dedicated_network_policy)
        .await
        .map_err(|e| {
            error!(
                "Failed to apply network policy: {} in namespace: {}. Error: {:?}",
                policy_name, namespace, e
            );
            OperatorError::NetworkPolicyError(format!("Failed to apply network policy: {:?}", e))
        })?;

    info!(
        "Successfully applied network policy: {} in namespace: {}",
        policy_name, namespace
    );
    Ok(())
}

/// Reconcile the IngressRouteTCP resources for dedicated networking.
///
/// This function creates or deletes IngressRouteTCP resources for both the primary and standby
/// services based on the dedicated networking configuration.
///
/// # Parameters
/// - `cdb`: The CoreDB custom resource instance.
/// - `ctx`: The operator context containing the Kubernetes client and other configurations.
/// - `namespace`: The namespace in which to apply the ingress routes.
/// - `basedomain`: The base domain for the ingress routes.
/// - `ingress_name_prefix`: The prefix for the ingress resource names.
/// - `service_name`: The name of the service associated with the ingress route.
/// - `port`: The port for the service.
/// - `middleware_names`: A list of middleware names to associate with the ingress route.
/// - `delete`: Whether to delete the ingress route.
/// - `is_standby`: Whether the ingress route is for a standby service.
#[allow(clippy::too_many_arguments)]
async fn reconcile_dedicated_networking_ing_route_tcp(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    namespace: &str,
    basedomain: &str,
    ingress_name_prefix: &str,
    service_name: &str,
    port: IntOrString,
    middleware_names: Vec<String>,
    delete: bool,
    is_standby: bool,
) -> Result<(), OperatorError> {
    let subdomain = if is_standby {
        format!("dedicated-ro.{}", namespace)
    } else {
        format!("dedicated.{}", namespace)
    };

    let action = if delete { "Deleting" } else { "Applying" };
    info!(
        "{} IngressRouteTCP for service: {} in namespace: {}",
        action, service_name, namespace
    );

    reconcile_postgres_ing_route_tcp(
        cdb,
        ctx,
        &subdomain,
        basedomain,
        namespace,
        ingress_name_prefix,
        service_name,
        port,
        middleware_names,
        delete,
    )
    .await
}

/// Reconcile the Service resource for dedicated networking.
///
/// This function creates or deletes a Service resource for the primary or standby service
/// based on the dedicated networking configuration.
///
/// # Parameters
/// - `client`: The Kubernetes client.
/// - `namespace`: The namespace in which to create the service.
/// - `cdb_name`: The name of the CoreDB instance.
/// - `is_public`: Whether the service is public or private.
/// - `is_standby`: Whether the service is for a standby (read-only) instance.
async fn reconcile_dedicated_networking_service(
    client: Client,
    namespace: &str,
    cdb_name: &str,
    is_public: bool,
    is_standby: bool,
) -> Result<(), OperatorError> {
    let service_name = if is_standby {
        format!("{}-dedicated-ro", cdb_name)
    } else {
        format!("{}-dedicated", cdb_name)
    };

    let lb_scheme = if is_public {
        "internet-facing"
    } else {
        "internal"
    };
    let lb_internal = if is_public { "false" } else { "true" };

    info!(
        "Applying Service: {} in namespace: {} with scheme: {}",
        service_name, namespace, lb_scheme
    );

    let service = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": service_name,
            "namespace": namespace,
            "annotations": {
                "external-dns.alpha.kubernetes.io/hostname": format!("{}.{}", service_name, "example.com"),
                "service.beta.kubernetes.io/aws-load-balancer-internal": lb_scheme,
                "service.beta.kubernetes.io/aws-load-balancer-scheme": lb_internal,
                "service.beta.kubernetes.io/aws-load-balancer-nlb-target-type": "ip",
                "service.beta.kubernetes.io/aws-load-balancer-type": "nlb-ip",
                "service.beta.kubernetes.io/aws-load-balancer-healthcheck-protocol": "TCP",
                "service.beta.kubernetes.io/aws-load-balancer-healthcheck-port": "5432"
            },
            "labels": {
                "cnpg.io/cluster": cdb_name
            }
        },
        "spec": {
            "loadBalancerSourceRanges": [ /* Add your IP allow list here */ ],
            "ports": [{
                "name": "postgres",
                "port": 5432,
                "protocol": "TCP",
                "targetPort": 5432
            }],
            "selector": {
                "cnpg.io/cluster": cdb_name,
                "role": if is_standby { "replica" } else { "primary" }
            },
            "sessionAffinity": "None",
            "type": "LoadBalancer"
        }
    });

    let svc_api: Api<Service> = Api::namespaced(client, namespace);
    let patch_params = PatchParams::apply("conductor").force();
    let patch = Patch::Apply(&service);

    // Applying the service
    svc_api
        .patch(&service_name, &patch_params, &patch)
        .await
        .map_err(|e| {
            error!(
                "Failed to apply service: {} in namespace: {}. Error: {}",
                service_name, namespace, e
            );
            OperatorError::ServiceError(format!("Failed to apply service: {:?}", e))
        })?;

    info!(
        "Successfully applied service: {} in namespace: {}",
        service_name, namespace
    );

    Ok(())
}

/// Delete the Service resource for dedicated networking.
///
/// This function deletes the Service resource for either the primary or standby service.
///
/// # Parameters
/// - `client`: The Kubernetes client.
/// - `namespace`: The namespace in which to delete the service.
/// - `cdb_name`: The name of the CoreDB instance.
/// - `is_standby`: Whether the service is for a standby (read-only) instance.
async fn delete_dedicated_networking_service(
    client: Client,
    namespace: &str,
    cdb_name: &str,
    is_standby: bool,
) -> Result<(), OperatorError> {
    let service_name = if is_standby {
        format!("{}-dedicated-ro", cdb_name)
    } else {
        format!("{}-dedicated", cdb_name)
    };

    let svc_api: Api<Service> = Api::namespaced(client, namespace);

    info!(
        "Checking if service: {} exists in namespace: {} for deletion",
        service_name, namespace
    );

    if svc_api.get(&service_name).await.is_ok() {
        info!(
            "Service: {} exists in namespace: {}. Proceeding with deletion.",
            service_name, namespace
        );

        svc_api
            .delete(&service_name, &Default::default())
            .await
            .map_err(|e| {
                error!(
                    "Failed to delete service: {} in namespace: {}. Error: {}",
                    service_name, namespace, e
                );
                OperatorError::ServiceError(format!("Failed to delete service: {:?}", e))
            })?;

        info!(
            "Successfully deleted service: {} in namespace: {}",
            service_name, namespace
        );
    } else {
        debug!(
            "Service: {} does not exist in namespace: {}. Skipping deletion.",
            service_name, namespace
        );
    }

    Ok(())
}
