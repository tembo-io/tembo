use k8s_openapi::api::{
    core::v1::{Endpoints, Service},
    networking::v1::NetworkPolicy,
};
use kube::{
    api::{Patch, PatchParams},
    runtime::controller::Action,
    Api, Client,
};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error};

pub async fn reconcile_network_policies(client: Client, namespace: &str) -> Result<(), Action> {
    let kubernetes_api_ip_addresses = lookup_kubernetes_api_ips(&client).await?;

    let np_api: Api<NetworkPolicy> = Api::namespaced(client, namespace);

    // Deny any network ingress or egress unless allowed
    // by another Network Policy
    let deny_all = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": format!("deny-all"),
            "namespace": format!("{namespace}"),
        },
        "spec": {
            "podSelector": {},
            "policyTypes": [
                "Egress",
                "Ingress"
            ],
        }
    });
    apply_network_policy(namespace, &np_api, deny_all).await?;

    let allow_dns = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": "allow-egress-to-dns",
            "namespace": format!("{namespace}"),
        },
        "spec": {
            "podSelector": {},
            "policyTypes": [
                "Egress"
            ],
            "egress": [
                {
                    "to": [
                        {
                            "namespaceSelector": {
                                "matchLabels": {
                                    "kubernetes.io/metadata.name": "kube-system"
                                }
                            },
                            "podSelector": {
                                "matchLabels": {
                                    "k8s-app": "node-local-dns"
                                }
                            }
                        }
                    ],
                    "ports": [
                        {
                            "protocol": "UDP",
                            "port": 53
                        },
                        {
                            "protocol": "TCP",
                            "port": 53
                        }
                    ]
                },
                {
                    "to": [
                        {
                            "namespaceSelector": {
                                "matchLabels": {
                                    "kubernetes.io/metadata.name": "kube-system"
                                }
                            },
                            "podSelector": {
                                "matchLabels": {
                                    "k8s-app": "kube-dns"
                                }
                            }
                        }
                    ],
                    "ports": [
                        {
                            "protocol": "UDP",
                            "port": 53
                        },
                        {
                            "protocol": "TCP",
                            "port": 53
                        }
                    ]
                }
            ]
        }
    });
    apply_network_policy(namespace, &np_api, allow_dns).await?;

    // Namespaces that should be allowed to access an instance namespace
    let allow_system_ingress = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
          "name": "allow-system",
          "namespace": format!("{namespace}"),
        },
        "spec": {
          "podSelector": {},
          "policyTypes": ["Ingress"],
          "ingress": [
            {
              "from": [
                {
                  "namespaceSelector": {
                    "matchLabels": {
                      "kubernetes.io/metadata.name": "monitoring"
                    }
                  }
                },
                {
                  "namespaceSelector": {
                    "matchLabels": {
                      "kubernetes.io/metadata.name": "cnpg-system"
                    }
                  }
                },
                {
                  "namespaceSelector": {
                    "matchLabels": {
                      "kubernetes.io/metadata.name": "coredb-operator"
                    }
                  }
                },
                {
                  "namespaceSelector": {
                    "matchLabels": {
                      "kubernetes.io/metadata.name": "traefik"
                    }
                  }
                },
                {
                  "namespaceSelector": {
                    "matchLabels": {
                      "kubernetes.io/metadata.name": "tembo-system"
                    }
                  }
                }
              ]
            }
          ]
        }
    });
    apply_network_policy(namespace, &np_api, allow_system_ingress).await?;

    // Namespaces that should be accessible from instance namespaces
    let allow_system_egress = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
          "name": "allow-system-egress",
          "namespace": format!("{namespace}"),
        },
        "spec": {
          "podSelector": {},
          "policyTypes": ["Egress"],
          "egress": [
            {
              "to": [
                {
                  "namespaceSelector": {
                    "matchLabels": {
                      "kubernetes.io/metadata.name": "minio"
                    }
                  }
                }
              ]
            },
            {
              "to": [
                {
                  "namespaceSelector": {
                    "matchLabels": {
                      "kubernetes.io/metadata.name": "traefik"
                    }
                  }
                }
              ],
              "ports": [
                {
                  "protocol": "TCP",
                  "port": 443
                },
                {
                  "protocol": "TCP",
                  "port": 8443
                }
              ]
            }
          ]
        }
    });
    apply_network_policy(namespace, &np_api, allow_system_egress).await?;

    let allow_public_internet = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
          "name": "allow-egress-to-internet",
          "namespace": format!("{namespace}"),
        },
        "spec": {
          "podSelector": {},
          "policyTypes": ["Egress"],
          "egress": [
            {
              "to": [
                {
                  "ipBlock": {
                    "cidr": "0.0.0.0/0",
                    "except": [
                      "10.0.0.0/8",
                      "172.16.0.0/12",
                      "192.168.0.0/16"
                    ]
                  }
                }
              ]
            }
          ]
        }
    });
    apply_network_policy(namespace, &np_api, allow_public_internet).await?;

    let allow_within_namespace = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
          "name": "allow-within-namespace",
          "namespace": format!("{namespace}"),
        },
        "spec": {
          "podSelector": {},
          "policyTypes": ["Ingress", "Egress"],
          "ingress": [
            {
              "from": [
                {
                  "podSelector": {}
                }
              ]
            }
          ],
          "egress": [
            {
              "to": [
                {
                  "podSelector": {}
                }
              ]
            }
          ]
        }
    });
    apply_network_policy(namespace, &np_api, allow_within_namespace).await?;

    let mut ip_list_kube_api = Vec::new();
    for ip_address in kubernetes_api_ip_addresses {
        ip_list_kube_api.push(serde_json::json!({
            "ipBlock": {
                "cidr": format!("{}/32", ip_address)
            }
        }));
    }

    let allow_kube_api = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
          "name": "allow-kube-api",
          "namespace": format!("{namespace}"),
        },
        "spec": {
          "podSelector": {},
          "policyTypes": ["Egress"],
          "egress": [
            {
              "to": ip_list_kube_api
            }
          ]
        }
    });
    apply_network_policy(namespace, &np_api, allow_kube_api).await?;

    let allow_proxy_to_access_tembo_ai_gateway = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": "allow-proxy-to-access-tembo-ai-gateway",
            "namespace": namespace,
        },
        "spec": {
            "podSelector": {
                "matchLabels": {
                    "app": format!("{}-ai-proxy", namespace)
                }
            },
            "policyTypes": ["Egress"],
            "egress": [
                {
                    "to": [
                        {
                            "namespaceSelector": {
                                "matchLabels": {
                                    "kubernetes.io/metadata.name": "tembo-ai"
                                }
                            },
                            "podSelector": {
                                "matchLabels": {
                                    "app.kubernetes.io/name": "tembo-ai-gateway"
                                }
                            }
                        }
                    ]
                }
            ]
        }
    });

    apply_network_policy(namespace, &np_api, allow_proxy_to_access_tembo_ai_gateway).await?;

    let allow_proxy_to_access_tembo_ai_gateway_internal_lb = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
          "name": "allow-proxy-to-access-tembo-ai-gateway-internal-lb",
          "namespace": format!("{namespace}"),
        },
        "spec": {
          "podSelector": {
            "matchLabels": {
              "app": format!("{}-ai-proxy", namespace)
            }
          },
          "policyTypes": ["Egress"],
          "egress": [
            {
              "ports": [
                {
                  "port": 8080,
                  "protocol": "TCP"
                }
              ],
              "to": [
                {
                  "ipBlock": {
                    "cidr": "10.0.0.0/8"
                  }
                }
              ]
            }
          ]
        }
    });

    apply_network_policy(
        namespace,
        &np_api,
        allow_proxy_to_access_tembo_ai_gateway_internal_lb,
    )
    .await?;
    Ok(())
}

// This function essentially does
// kubectl get svc -n default kubernetes
// kubectl get endpoints -n default kubernetes
// To return the IP addresses of the kubernetes API server
async fn lookup_kubernetes_api_ips(client: &Client) -> Result<Vec<String>, Action> {
    let service_api = Api::<Service>::namespaced(client.clone(), "default");
    // Look up IP address of 'kubernetes' service in default namespace
    let kubernetes_service = match service_api.get("kubernetes").await {
        Ok(s) => s,
        Err(_) => {
            error!("Failed to get kubernetes service");
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    let kubernetes_service_spec = match kubernetes_service.spec {
        Some(s) => s,
        None => {
            error!("while discovering kubernetes API IP address, service has no spec");
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    let cluster_ip = match kubernetes_service_spec.cluster_ip.clone() {
        Some(c) => c,
        None => {
            error!("while discovering kubernetes API IP address, service has no cluster IP");
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    let mut results = Vec::new();
    results.push(cluster_ip);
    let endpoints_api = Api::<Endpoints>::namespaced(client.clone(), "default");
    let kubernetes_endpoint = match endpoints_api.get("kubernetes").await {
        Ok(endpoint) => endpoint,
        Err(e) => {
            error!("Failed to get kubernetes endpoint: {}", e);
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    let kubernetes_endpoint_subsets = match kubernetes_endpoint.subsets {
        Some(s) => s,
        None => {
            error!("while discovering kubernetes API IP address, endpoint has no subsets");
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    if kubernetes_endpoint_subsets.is_empty() {
        error!("While discovering kubernetes API IP address, found no endpoints");
        return Err(Action::requeue(Duration::from_secs(300)));
    }
    for subset in kubernetes_endpoint_subsets {
        let addresses = match subset.addresses {
            Some(a) => a,
            None => {
                error!(
                    "while discovering kubernetes API IP address, endpoint subset has no addresses"
                );
                return Err(Action::requeue(Duration::from_secs(300)));
            }
        };
        for address in addresses {
            results.push(address.ip);
        }
    }
    results.sort();
    Ok(results)
}

pub async fn apply_network_policy(
    namespace: &str,
    np_api: &Api<NetworkPolicy>,
    np: Value,
) -> Result<(), Action> {
    let network_policy: NetworkPolicy = match serde_json::from_value(np) {
        Ok(np) => np,
        Err(_) => {
            error!(
                "Failed to deserialize Network Policy namespace {}",
                namespace
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    let name = network_policy.metadata.name.as_ref().ok_or_else(|| {
        error!(
            "Network policy name is empty in namespace: {}.",
            namespace.to_string()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let params: PatchParams = PatchParams::apply("conductor").force();
    debug!(
        "\nApplying Network Policy {} in namespace {}",
        name, namespace
    );
    let _o: NetworkPolicy = match np_api
        .patch(name, &params, &Patch::Apply(&network_policy))
        .await
    {
        Ok(np) => np,
        Err(_) => {
            error!(
                "Failed to create Network Policy {} in namespace {}",
                name, namespace
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    Ok(())
}
