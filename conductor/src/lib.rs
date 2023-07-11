pub mod aws;
pub mod errors;
pub mod extensions;
pub mod types;

use crate::aws::cloudformation::{AWSConfigState, CloudFormationParams};
use aws_sdk_cloudformation::config::Region;
use controller::{
    apis::coredb_types::{CoreDB, CoreDBSpec},
    cloudnativepg::clusters::Cluster,
};
use errors::ConductorError;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::{Namespace, Secret};
use k8s_openapi::api::networking::v1::NetworkPolicy;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};

use chrono::{SecondsFormat, Utc};
use kube::{Api, Client};
use log::{debug, info};
use rand::Rng;
use serde_json::{from_str, to_string, Value};

pub type Result<T, E = ConductorError> = std::result::Result<T, E>;

pub async fn generate_spec(namespace: &str, spec: &CoreDBSpec) -> Value {
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
pub async fn get_coredb_status(client: Client, namespace: &str) -> Result<CoreDB, ConductorError> {
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

async fn get_secret_for_db(client: Client, name: &str) -> Result<Secret, ConductorError> {
    // read secret <name>-connection
    let secret_name_cdb = format!("{name}-connection");
    let secret_name_cnpg = format!("{name}-superuser");

    let secret_api: Api<Secret> = Api::namespaced(client, name);

    if let Some(secret) = secret_api.get_opt(secret_name_cnpg.as_str()).await? {
        debug!("Found the secret {}", secret_name_cnpg);
        Ok(secret)
    } else {
        debug!(
            "Didn't find the secret {}, trying cdb-style {}",
            secret_name_cnpg, secret_name_cdb
        );
        if let Some(secret) = secret_api.get_opt(secret_name_cdb.as_str()).await? {
            debug!("Found the secret {}", secret_name_cdb);
            Ok(secret)
        } else {
            debug!("Didn't find the secret {}", secret_name_cdb);
            Err(ConductorError::PostgresConnectionInfoNotFound)
        }
    }
}

pub async fn get_pg_conn(
    client: Client,
    name: &str,
    basedomain: &str,
) -> Result<types::ConnectionInfo, ConductorError> {
    let secret = get_secret_for_db(client, name).await?;

    let data = secret.data.unwrap();

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

pub async fn restart_statefulset(
    client: Client,
    namespace: &str,
    statefulset_name: &str,
) -> Result<(), ConductorError> {
    let sts: Api<StatefulSet> = Api::namespaced(client, namespace);
    sts.restart(statefulset_name).await?;
    Ok(())
}

pub async fn restart_cnpg(
    client: Client,
    namespace: &str,
    cluster_name: &str,
) -> Result<(), ConductorError> {
    let cluster: Api<Cluster> = Api::namespaced(client, namespace);
    let restart = Utc::now()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
        .to_string();

    // To restart the CNPG pod we need to annotate the Cluster resource with
    // kubectl.kubernetes.io/restartedAt: <timestamp>
    let patch_json = serde_json::json!({
        "metadata": {
            "annotations": {
                "kubectl.kubernetes.io/restartedAt": restart
            }
        }
    });

    // Use the patch method to update the Cluster resource
    let params = PatchParams::default();
    let _patch = cluster
        .patch(cluster_name, &params, &Patch::Merge(patch_json))
        .await
        .map_err(ConductorError::KubeError)?;
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
    // When moving this into operator, handle the specific errors that mean
    // "cloudformation is not done yet" and return a more specific error
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
