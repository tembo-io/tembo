use crate::{apis::coredb_types::CoreDB, Context};
use crate::{errors::OperatorError, network_policies::apply_network_policy};
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::networking::v1::NetworkPolicy;
use kube::Resource;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt},
    client::Client,
};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tracing::{debug, error, info};

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
    basedomain: &str,
) -> Result<(), OperatorError> {
    let ns = cdb.namespace().unwrap_or_else(|| {
        error!(
            "Namespace not found for CoreDB instance: {}",
            cdb.name_any()
        );
        "default".to_string()
    });
    let client = ctx.client.clone();

    debug!(
        "Starting reconciliation of dedicated networking for CoreDB instance: {} in namespace: {}",
        cdb.name_any(),
        ns
    );

    if let Some(dedicated_networking) = &cdb.spec.dedicatedNetworking {
        if dedicated_networking.enabled {
            debug!(
                "Dedicated networking is enabled for CoreDB instance: {}",
                cdb.name_any()
            );

            debug!(
                "Reconciling network policies for dedicated networking in namespace: {}",
                ns
            );
            reconcile_dedicated_networking_network_policies(cdb.clone(), client.clone(), &ns)
                .await
                .map_err(|e| {
                    error!("Failed to reconcile network policies: {:?}", e);
                    e
                })?;
            debug!(
                "Handling primary service ingress for CoreDB instance: {}",
                cdb.name_any()
            );
            reconcile_dedicated_networking_service(
                cdb,
                client.clone(),
                &ns,
                dedicated_networking.public,
                false,
                &dedicated_networking.serviceType,
                basedomain,
            )
            .await
            .map_err(|e| {
                error!("Failed to reconcile primary service ingress: {:?}", e);
                e
            })?;

            if dedicated_networking.includeStandby {
                debug!(
                    "Handling standby service ingress for CoreDB instance: {}",
                    cdb.name_any()
                );
                reconcile_dedicated_networking_service(
                    cdb,
                    client.clone(),
                    &ns,
                    dedicated_networking.public,
                    true,
                    &dedicated_networking.serviceType,
                    basedomain,
                )
                .await
                .map_err(|e| {
                    error!("Failed to reconcile standby service ingress: {:?}", e);
                    e
                })?;
            } else {
                debug!(
                    "Standby service is not included. Deleting standby service for CoreDB instance: {}",
                    cdb.name_any()
                );
                delete_dedicated_networking_service(client.clone(), &ns, &cdb.name_any(), true)
                    .await
                    .map_err(|e| {
                        error!("Failed to delete standby service: {:?}", e);
                        e
                    })?;
            }
        } else {
            debug!(
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
        }
    } else {
        debug!(
            "Dedicated networking is not configured for CoreDB instance: {}",
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
    }

    debug!(
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
    cdb: CoreDB,
    client: Client,
    namespace: &str,
) -> Result<(), OperatorError> {
    let cdb_name = cdb.name_any();
    let np_api: Api<NetworkPolicy> = Api::namespaced(client, namespace);

    let cidr_list = env::var("CLOUD_LOAD_BALANCER_INTERNAL_IP_CIDR")
        .unwrap_or_else(|_| "10.0.0.0/8".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<String>>();

    let policy_name = format!("{}-allow-nlb", cdb_name);
    debug!(
        "Applying network policy: {} in namespace: {} to allow traffic from CIDRs: {:?}",
        policy_name, namespace, cidr_list
    );

    let ingress_rules = cidr_list
        .into_iter()
        .map(|cidr| {
            serde_json::json!({
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
            })
        })
        .collect::<Vec<_>>();

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
            "ingress": ingress_rules
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

    debug!(
        "Successfully applied network policy: {} in namespace: {}",
        policy_name, namespace
    );
    Ok(())
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
    cdb: &CoreDB,
    client: Client,
    namespace: &str,
    is_public: bool,
    is_standby: bool,
    service_type: &str,
    basedomain: &str,
) -> Result<(), OperatorError> {
    let cdb_name = &cdb.name_any();
    let owner_reference = cdb.controller_owner_ref(&()).unwrap();
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

    debug!(
        "Applying Service: {} in namespace: {} with type: {} and scheme: {}",
        service_name, namespace, service_type, lb_scheme
    );

    let mut annotations = serde_json::Map::new();
    annotations.insert(
        "external-dns.alpha.kubernetes.io/hostname".to_string(),
        serde_json::Value::String(format!("{}.{}", namespace, basedomain)),
    );

    annotations.extend([
        (
            "service.beta.kubernetes.io/aws-load-balancer-internal".to_string(),
            serde_json::Value::String(lb_scheme.to_string()),
        ),
        (
            "service.beta.kubernetes.io/aws-load-balancer-scheme".to_string(),
            serde_json::Value::String(lb_internal.to_string()),
        ),
        (
            "service.beta.kubernetes.io/aws-load-balancer-nlb-target-type".to_string(),
            serde_json::Value::String("ip".to_string()),
        ),
        (
            "service.beta.kubernetes.io/aws-load-balancer-type".to_string(),
            serde_json::Value::String("nlb-ip".to_string()),
        ),
        (
            "service.beta.kubernetes.io/aws-load-balancer-healthcheck-protocol".to_string(),
            serde_json::Value::String("TCP".to_string()),
        ),
        (
            "service.beta.kubernetes.io/aws-load-balancer-healthcheck-port".to_string(),
            serde_json::Value::String("5432".to_string()),
        ),
    ]);

    let mut labels = serde_json::Map::new();
    labels.insert(
        "cnpg.io/cluster".to_string(),
        serde_json::Value::String(cdb_name.to_string()),
    );
    if is_public {
        labels.insert(
            "public".to_string(),
            serde_json::Value::String("true".to_string()),
        );
    }

    let mut service_spec = serde_json::Map::new();
    service_spec.insert(
        "ports".to_string(),
        json!([{
            "name": "postgres",
            "port": 5432,
            "protocol": "TCP",
            "targetPort": 5432
        }]),
    );
    service_spec.insert(
        "selector".to_string(),
        json!({
            "cnpg.io/cluster": cdb_name,
            "role": if is_standby { "replica" } else { "primary" }
        }),
    );
    service_spec.insert("sessionAffinity".to_string(), json!("None"));
    service_spec.insert("type".to_string(), json!(service_type));

    if service_type == "LoadBalancer" {
        let load_balancer_source_ranges = env::var("CLOUD_LOAD_BALANCER_INTERNAL_IP_CIDR")
            .unwrap_or_else(|_| "10.0.0.0/8".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>();

        service_spec.insert(
            "loadBalancerSourceRanges".to_string(),
            json!(load_balancer_source_ranges),
        );
    }

    let service = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": service_name,
            "namespace": namespace,
            "annotations": annotations,
            "ownerReferences": [owner_reference],
            "labels": labels
        },
        "spec": service_spec
    });

    let svc_api: Api<Service> = Api::namespaced(client, namespace);
    let patch_params = PatchParams::apply("cntrlr").force();
    let patch = Patch::Apply(&service);

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

    debug!(
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

    debug!(
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
