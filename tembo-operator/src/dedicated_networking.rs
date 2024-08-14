use std::sync::Arc;

use k8s_openapi::{
    api::core::v1::{Namespace, Pod},
    apimachinery::pkg::util::intstr::IntOrString,
};
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
        wait::Condition,
        watcher::Config as watcherConfig,
    },
    Resource,
};
use std::env;
use crate::{apis::coredb_types::CoreDB, Context, Error};
use serde_json::json;
use tracing::{debug, error, info};
use k8s_openapi::api::networking::v1::NetworkPolicy;

use crate::{
    config::Config,
    errors::OperatorError,
    ingress::{
        reconcile_postgres_ing_route_tcp, reconcile_ip_allowlist_middleware,
    },
    network_policies::{apply_network_policy, reconcile_network_policies},
};
use k8s_openapi::api::core::v1::Service;
use crate::{
    apis::postgres_parameters::PgConfig,
    configmap::reconcile_generic_metrics_configmap,
    extensions::{database_queries::list_config_params, reconcile_extensions},
    ingress::reconcile_extra_postgres_ing_route_tcp,
    postgres_exporter::reconcile_metrics_configmap,
    trunk::{extensions_that_require_load, reconcile_trunk_configmap},
};

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
    let ns = cdb.namespace().unwrap();
    let basedomain = env::var("DATA_PLANE_BASEDOMAIN").unwrap_or_else(|_| "localhost".to_string()); // Replace with dynamic retrieval if needed
    let port = IntOrString::Int(5432);
    let client = ctx.client.clone();

    info!("Starting reconciliation of dedicated networking for CoreDB instance: {}", cdb.name_any());

    // Check if dedicated networking is enabled in the CoreDB spec
    if let Some(dedicated_networking) = &cdb.spec.dedicated_networking {
        if dedicated_networking.enabled {
            info!("Dedicated networking is enabled for CoreDB instance: {}", cdb.name_any());

            // Reconcile network policies specific to dedicated networking
            info!("Reconciling network policies for dedicated networking in namespace: {}", ns);
            reconcile_dedicated_networking_network_policies(
                client.clone(),
                &ns,
                &cdb.name_any(),
                "10.0.0.0/8", // Replace with the actual CIDR for your dedicated network
            )
            .await?;

            // Reconcile the IP allow list middleware
            info!("Reconciling IP allow list middleware for CoreDB instance: {}", cdb.name_any());
            let middleware_name = reconcile_ip_allowlist_middleware(cdb, ctx.clone()).await?;

            // Handle primary service ingress
            info!("Handling primary service ingress for CoreDB instance: {}", cdb.name_any());
            reconcile_dedicated_networking_service(
                client.clone(),
                &ns,
                &cdb.name_any(),
                dedicated_networking.public,
                false, // primary service
            )
            .await?;

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
            .await?;

            // Handle standby service ingress if `include_standby` is true
            if dedicated_networking.include_standby {
                info!("Handling standby service ingress for CoreDB instance: {}", cdb.name_any());
                reconcile_dedicated_networking_service(
                    client.clone(),
                    &ns,
                    &cdb.name_any(),
                    dedicated_networking.public,
                    true, // standby service
                )
                .await?;

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
                    true, // standby service
                )
                .await?;
            }
        } else {
            info!(
                "Deleting dedicated networking services for CoreDB instance: {} as dedicated networking is disabled",
                cdb.name_any()
            );
            // Delete the services if dedicated networking is disabled
            delete_dedicated_networking_service(
                client.clone(),
                &ns,
                &cdb.name_any(),
                false, // primary service
            ).await?;

            delete_dedicated_networking_service(
                client.clone(),
                &ns,
                &cdb.name_any(),
                true, // standby service
            ).await?;

            reconcile_dedicated_networking_ing_route_tcp(
                cdb,
                ctx.clone(),
                &ns,
                &basedomain,
                "dedicated",
                &format!("{}-dedicated", cdb.name_any()),
                port.clone(),
                vec![],
                true, // delete
                false, // primary service
            )
            .await?;

            reconcile_dedicated_networking_ing_route_tcp(
                cdb,
                ctx,
                &ns,
                &basedomain,
                "dedicated-ro",
                &format!("{}-dedicated-ro", cdb.name_any()),
                port,
                vec![],
                true, // delete
                true, // standby service
            )
            .await?;
        }
    }

    info!("Completed reconciliation of dedicated networking for CoreDB instance: {}", cdb.name_any());
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

    // Define a new NetworkPolicy for the dedicated networking
    let dedicated_network_policy = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": format!("{}-allow-nlb", cdb_name),
            "namespace": format!("{namespace}"),
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
            "Failed to apply dedicated networking network policy: {:?}",
            e
        );
        OperatorError::NetworkPolicyError(format!("Failed to apply network policy: {:?}", e))
    })?;

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

    let lb_scheme = if is_public { "internet-facing" } else { "internal" };
    let lb_internal = if is_public { "false" } else { "true" };

    let service = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": service_name,
            "namespace": namespace,
            "annotations": {
                "external-dns.alpha.kubernetes.io/hostname": format!("{}.{}", service_name, "example.com"),
                "service.beta.kubernetes.io/aws-load-balancer-internal": lb_internal,
                "service.beta.kubernetes.io/aws-load-balancer-scheme": lb_scheme,
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
    svc_api.patch(&service_name, &patch_params, &patch).await.map_err(|e| {
        error!("Failed to apply service {}: {}", service_name, e);
        OperatorError::ServiceError(format!("Failed to apply network policy: {:?}", e))
    })?;

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
    if svc_api.get(&service_name).await.is_ok() {
        svc_api.delete(&service_name, &Default::default()).await.map_err(|e| {
            error!("Failed to delete service {}: {}", service_name, e);
            OperatorError::ServiceError(format!("Failed to apply network policy: {:?}", e))
        })?;
    } else {
        debug!("Service {} does not exist, skipping deletion", service_name);
    }

    Ok(())
}
