pub mod aws;
pub mod coredb_crd;
pub mod errors;
pub mod extensions;
mod ingress_route_tcp_crd;
pub mod types;

use crate::aws::cloudformation::{AWSConfigState, CloudFormationParams};
use aws_sdk_cloudformation::config::Region;
use base64::{engine::general_purpose, Engine as _};
use coredb_crd as crd;
use coredb_crd::CoreDB;
use errors::ConductorError;
use ingress_route_tcp_crd::IngressRouteTCP;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::{Namespace, Secret};
use k8s_openapi::api::networking::v1::{Ingress, NetworkPolicy};
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::runtime::wait::{await_condition, Condition};
use kube::{Api, Client};
use log::info;
use rand::Rng;
use serde_json::{from_str, to_string, Value};

pub type Result<T, E = ConductorError> = std::result::Result<T, E>;

pub async fn generate_spec(namespace: &str, spec: &crd::CoreDBSpec) -> Value {
    let spec = serde_json::json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "metadata": {
            "name": namespace,
        },
        "spec": spec,
    });
    spec
}

pub async fn create_ing_route_tcp(
    client: Client,
    name: &str,
    basedomain: &str,
) -> Result<(), ConductorError> {
    let ing_api: Api<IngressRouteTCP> = Api::namespaced(client, name);
    let params = PatchParams::apply("conductor").force();
    let ing = serde_json::json!({
        "apiVersion": "traefik.containo.us/v1alpha1",
        "kind": "IngressRouteTCP",
        "metadata": {
            "name": format!("{name}"),
            "namespace": format!("{name}"),
        },
        "spec": {
            "entryPoints": ["postgresql"],
            "routes": [
                {
                    "match": format!("HostSNI(`{name}.{basedomain}`)"),
                    "services": [
                        {
                            "name": format!("{name}"),
                            "port": 5432,
                        },
                    ],
                },
            ],
            "tls": {
                "passthrough": true,
            },
        },
    });
    info!("\nCreating or updating IngressRouteTCP: {}", name);
    let _o = ing_api
        .patch(name, &params, &Patch::Apply(&ing))
        .await
        .map_err(ConductorError::KubeError)?;
    Ok(())
}

pub async fn create_metrics_ingress(
    client: Client,
    name: &str,
    basedomain: &str,
) -> Result<(), ConductorError> {
    let ing_api: Api<Ingress> = Api::namespaced(client, name);
    let params = PatchParams::apply("conductor").force();
    let ingress = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "Ingress",
        "metadata": {
            "name": format!("{name}"),
            "namespace": format!("{name}"),
        },
        "spec": {
            "ingressClassName": "traefik",
            "rules": [
                {
                    "host": format!("{name}.{basedomain}"),
                    "http": {
                        "paths": [
                            {
                                "path": "/metrics",
                                "pathType": "Prefix",
                                "backend": {
                                    "service": {
                                        "name": format!("{name}-metrics"),
                                        "port": {
                                            "number": 80,
                                        },
                                    },
                                },
                            },
                        ],
                    },
                },
            ],
        },
    });

    info!("\nCreating or updating Ingress: {}", name);
    let _o = ing_api
        .patch(name, &params, &Patch::Apply(&ingress))
        .await
        .map_err(ConductorError::KubeError)?;
    Ok(())
}

pub async fn get_all(client: Client, namespace: &str) -> Vec<CoreDB> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let pg_list = coredb_api
        .list(&ListParams::default())
        .await
        .expect("could not get CoreDBs");
    pg_list.items
}

pub async fn get_one(client: Client, namespace: &str) -> Result<CoreDB, ConductorError> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let pg_instance = coredb_api.get(namespace).await?;
    Ok(pg_instance)
}

// returns CoreDB when status is present, otherwise returns an error
pub async fn get_coredb_status(
    client: Client,
    namespace: &str,
) -> Result<crd::CoreDB, ConductorError> {
    let coredb = get_one(client, namespace).await?;

    if coredb.status.is_none() {
        Err(ConductorError::NoStatusReported)
    } else {
        Ok(coredb)
    }
}

pub async fn create_or_update(
    client: Client,
    namespace: &str,
    deployment: serde_json::Value,
) -> Result<(), ConductorError> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let params = PatchParams::apply("conductor").force();
    let name: String = serde_json::from_value(deployment["metadata"]["name"].clone()).unwrap();
    info!("\nCreating or updating CoreDB: {}", name);
    let _ = coredb_api
        .patch(&name, &params, &Patch::Apply(&deployment))
        .await
        .map_err(ConductorError::KubeError)?;
    Ok(())
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), ConductorError> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let params = DeleteParams::default();
    info!("\nDeleting CoreDB: {}", name);
    let _o = coredb_api
        .delete(name, &params)
        .await
        .map_err(ConductorError::KubeError);
    Ok(())
}

pub async fn create_namespace(client: Client, name: &str) -> Result<(), ConductorError> {
    let ns_api: Api<Namespace> = Api::all(client);
    let params = PatchParams::apply("conductor").force();
    let ns = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": {
            "name": format!("{name}"),
        }
    });
    info!("\nCreating namespace {} if it does not exist", name);
    let _o = ns_api
        .patch(name, &params, &Patch::Apply(&ns))
        .await
        .map_err(ConductorError::KubeError)?;
    Ok(())
}

pub async fn create_networkpolicy(client: Client, name: &str) -> Result<(), ConductorError> {
    let np_api: Api<NetworkPolicy> = Api::namespaced(client, name);
    let params: PatchParams = PatchParams::apply("conductor").force();
    let np = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": format!("{name}"),
            "namespace": format!("{name}"),
        },
        "spec": {
            "podSelector": {
                "matchLabels": {
                    "app": "coredb",
                    "coredb.io/name": format!("{name}"),
                    "statefulset": format!("{name}")
                }
            },
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
                            }
                        },
                        {
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
                        }
                    ]
                },
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
                        },
                    ]
                }
            ]
        }
    });

    info!("\nCreating Network Policy {} if it does not exist", name);
    let _o: NetworkPolicy = np_api
        .patch(name, &params, &Patch::Apply(&np))
        .await
        .map_err(ConductorError::KubeError)?;

    Ok(())
}

pub async fn delete_namespace(client: Client, name: &str) -> Result<(), ConductorError> {
    let ns_api: Api<Namespace> = Api::all(client);
    let params = DeleteParams::default();
    info!("\nDeleting namespace: {}", name);
    let _ = ns_api
        .delete(name, &params)
        .await
        .map_err(ConductorError::KubeError);
    Ok(())
}

// remove after COR-166
#[allow(unused_variables)]
pub async fn get_pg_conn(
    client: Client,
    name: &str,
    basedomain: &str,
) -> Result<types::ConnectionInfo, ConductorError> {
    // read secret <name>-connection
    let secret_name = format!("{name}-connection");

    let secret_api: Api<Secret> = Api::namespaced(client, name);

    // wait for secret to exist
    let establish = await_condition(secret_api.clone(), &secret_name, wait_for_secret());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(90), establish).await;

    let secret = secret_api
        .get(secret_name.as_str())
        .await
        .expect("error getting Secret");

    let data = secret.data.unwrap();

    // TODO(ianstanton) There has to be a better way to do this
    let user_data = data.get("user").unwrap();
    let byte_user = to_string(user_data).unwrap();
    let string_user: String = from_str(&byte_user).unwrap();

    let pw_data = data.get("password").unwrap();
    let byte_pw = to_string(pw_data).unwrap();
    let string_pw: String = from_str(&byte_pw).unwrap();

    let host = format!("{name}.{basedomain}");

    Ok(types::ConnectionInfo {
        host,
        port: 5432,
        user: string_user,
        password: string_pw,
    })
}

#[allow(dead_code)]
fn b64_decode(b64_encoded: &str) -> String {
    let bytes = general_purpose::STANDARD.decode(b64_encoded).unwrap();
    std::str::from_utf8(&bytes).unwrap().to_owned()
}

// TODO(ianstanton) This is a hack for now. We need to find a more 'official' way of checking for
//  existing resources in the cluster.
pub fn wait_for_secret() -> impl Condition<Secret> {
    |obj: Option<&Secret>| {
        if let Some(secret) = &obj {
            if let Some(t) = &secret.type_ {
                return t == "Opaque";
            }
        }
        false
    }
}

pub async fn restart_statefulset(
    client: Client,
    namespace: &str,
    statefulset_name: &str,
) -> Result<(), ConductorError> {
    let sts: Api<StatefulSet> = Api::namespaced(client, namespace);
    sts.restart(statefulset_name).await?;
    Ok(())
}

// Create a cloudformation stack for the database.
// This will create an IAM role for the database to use to access the backup archive bucket
pub async fn create_cloudformation(
    aws_region: String,
    backup_archive_bucket: String,
    org_name: &str,
    db_name: &str,
    cf_template_bucket: &str,
) -> Result<(), ConductorError> {
    // (todo: nhudson) - Create Cloudformation Stack only for Create event
    // Create new function that returns 3 enums of SUCCESS, ERROR, WAITING
    // If status is something other than SUCCESS we would need to requeue the message
    // back to the queue.
    // If there is an error we will need to alert on it
    // If we are still waiting for the stack to be created we will need to requeue the message
    let region = Region::new(aws_region);
    let aws_config_state = AWSConfigState::new(region).await;
    let namespace = format!("org-{}-inst-{}", org_name, db_name);
    let stack_name = format!("org-{}-inst-{}-cf", org_name, db_name);
    let iam_role_name = format!("org-{}-inst-{}-iam", org_name, db_name);
    let service_account_name = format!("org-{}-inst-{}-sa", org_name, db_name);
    let cf_template_params = CloudFormationParams::new(
        // Database Backup Bucket Name
        String::from(&backup_archive_bucket),
        // Customer Org Name
        String::from(org_name),
        // Customer Database Name
        String::from(db_name),
        // AWS IAM Role Name to create
        String::from(&iam_role_name),
        // The AWS S3 Bucket where the CF Template is placed
        String::from(cf_template_bucket),
        // The Kubernetes Namespace where the database is deployed
        namespace,
        // The Kubernetes Service Account to use for the database
        String::from(&service_account_name),
    );
    aws_config_state
        .create_cloudformation_stack(&stack_name, &cf_template_params)
        .await
        .map_err(ConductorError::from)?;
    Ok(())

    // We will need to setup a requeuing system at somepoint to query for status
    // of the stack state.  If the stack is still in a CREATE_IN_PROGRESS or UPDATE_IN_PROGRESS
    // or DELETE_IN_PROGRESS we will need to requeue the message back to the queue.
    // We will also need to bubble up errors as an alert if there is a failure.
}

// Delete a cloudformation stack.
pub async fn delete_cloudformation(
    aws_region: String,
    org_name: &str,
    db_name: &str,
) -> Result<(), ConductorError> {
    let region = Region::new(aws_region);
    let aws_config_state = AWSConfigState::new(region).await;
    let stack_name = format!("org-{}-inst-{}-cf", org_name, db_name);
    aws_config_state
        .delete_cloudformation_stack(&stack_name)
        .await
        .map_err(ConductorError::from)?;
    Ok(())
}

pub struct StackOutputs {
    pub role_name: Option<String>,
    pub role_arn: Option<String>,
}

pub async fn lookup_role_arn(
    aws_region: String,
    organization_name: &str,
    dbname: &str,
) -> Result<String, ConductorError> {
    let stack_outputs = get_stack_outputs(aws_region, organization_name, dbname).await?;
    let role_arn = stack_outputs
        .role_arn
        .ok_or_else(|| ConductorError::NoOutputsFound)?;
    Ok(role_arn)
}

// Get Cloudformation Stack Outputs RoleName and RoleArn
async fn get_stack_outputs(
    aws_region: String,
    org_name: &str,
    db_name: &str,
) -> Result<StackOutputs, ConductorError> {
    let region = Region::new(aws_region);
    let aws_config_state = AWSConfigState::new(region).await;
    let stack_name = format!("org-{}-inst-{}-cf", org_name, db_name);
    let (role_name, role_arn) = aws_config_state
        .lookup_cloudformation_stack(&stack_name)
        .await
        .map_err(ConductorError::from)?;
    let stack_outputs = StackOutputs {
        role_name,
        role_arn,
    };
    Ok(stack_outputs)
}

pub async fn generate_rand_schedule() -> String {
    // Generate a random minute and hour between 4am and 10am UTC
    let mut rng = rand::thread_rng();
    let minute: u8 = rng.gen_range(0..60);
    let hour: u8 = rng.gen_range(4..10);

    format!("{} {} * * *", minute, hour)
}

#[test]
fn test_b64_decode_string() {
    let encoded = "SGVsbG8sIFdvcmxkIQ==";
    let decoded = b64_decode(encoded);
    assert_eq!(decoded, "Hello, World!");

    let encoded = "ZnJpZGF5";
    let decoded = b64_decode(encoded);
    assert_eq!(decoded, "friday");
}

#[test]
fn test_b64_decode_empty_string() {
    let encoded = "";
    let decoded = b64_decode(encoded);
    assert_eq!(decoded, "");
}
