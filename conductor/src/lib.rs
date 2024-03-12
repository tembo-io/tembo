pub mod aws;
pub mod errors;
pub mod extensions;
pub mod monitoring;
pub mod routes;
pub mod types;

use crate::aws::cloudformation::{AWSConfigState, CloudFormationParams};
use aws_sdk_cloudformation::config::Region;
use controller::apis::coredb_types::{CoreDB, CoreDBSpec};
use errors::ConductorError;

use k8s_openapi::api::core::v1::{Namespace, Secret};

use kube::api::{DeleteParams, ListParams, Patch, PatchParams};

use chrono::{DateTime, SecondsFormat, Utc};
use kube::{Api, Client, ResourceExt};
use log::{debug, info};
use serde_json::{from_str, to_string, Value};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

pub type Result<T, E = ConductorError> = std::result::Result<T, E>;

pub async fn generate_spec(
    org_id: &str,
    entity_name: &str,
    instance_id: &str,
    data_plane_id: &str,
    namespace: &str,
    spec: &CoreDBSpec,
) -> Value {
    serde_json::json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "metadata": {
            "name": namespace,
            "annotations": {
                "tembo.io/org_id": org_id,
                "tembo.io/instance_id": instance_id,
                "tembo.io/entity_name": entity_name,
                "tembo.io/data_plane_id": data_plane_id,
            }
        },
        "spec": spec,
    })
}

pub fn get_data_plane_id_from_coredb(coredb: &CoreDB) -> Result<String, Box<ConductorError>> {
    let annotations = match coredb.metadata.annotations.as_ref() {
        None => {
            return Err(Box::new(ConductorError::EventIDFormat));
        }
        Some(annotations) => annotations,
    };
    let data_plane_id = match annotations.get("tembo.io/data_plane_id") {
        Some(data_plane_id) => data_plane_id.to_string(),
        None => {
            return Err(Box::new(ConductorError::EventIDFormat));
        }
    };
    Ok(data_plane_id)
}

pub fn get_org_inst_id(coredb: &CoreDB) -> Result<types::OrgInstId, Box<ConductorError>> {
    let annotations = match coredb.metadata.annotations.as_ref() {
        None => {
            return Err(Box::new(ConductorError::EventIDFormat));
        }
        Some(annotations) => annotations,
    };
    let org_id = match annotations.get("tembo.io/org_id") {
        Some(org_id) => org_id.to_string(),
        None => {
            return Err(Box::new(ConductorError::EventIDFormat));
        }
    };
    let instance_id = match annotations.get("tembo.io/instance_id") {
        Some(instance_id) => instance_id.to_string(),
        None => {
            return Err(Box::new(ConductorError::EventIDFormat));
        }
    };
    Ok(types::OrgInstId {
        org_id,
        inst_id: instance_id,
    })
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
pub async fn get_coredb_error_without_status(
    client: Client,
    namespace: &str,
) -> Result<CoreDB, ConductorError> {
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
    let _ = coredb_api
        .delete(name, &params)
        .await
        .map_err(ConductorError::KubeError);

    Ok(())
}

pub async fn create_namespace(
    client: Client,
    name: &str,
    organization_id: &str,
    instance_id: &str,
) -> Result<(), ConductorError> {
    let ns_api: Api<Namespace> = Api::all(client);
    // check if the namespace already exists
    let params = ListParams::default().fields(&format!("metadata.name={}", name));
    let ns_list = ns_api.list(&params).await?;
    if !ns_list.items.is_empty() {
        return Ok(());
    }

    info!("\nCreating new namespace {}", name);
    let params = PatchParams::apply("conductor");
    // If the namespace already exists, do not include the label "tembo-pod-init.tembo.io/watch"
    // If it's a new namespace, include the label
    let ns = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": {
            "name": format!("{name}"),
            "labels": {
                "tembo-pod-init.tembo.io/watch": "true",
                "tembo.io/instance_id": instance_id,
                "tembo.io/organization_id": organization_id
            }
        }
    });
    info!("\nCreating namespace {}", name);
    let _o = ns_api
        .patch(name, &params, &Patch::Apply(&ns))
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

async fn get_secret_for_db(client: Client, name: &str) -> Result<(Secret, Secret), ConductorError> {
    // read secret <name>-connection
    let secret_name_cnpg_postgres = format!("{name}-connection");
    let secret_name_cnpg_app = format!("{name}-app");

    let secret_api: Api<Secret> = Api::namespaced(client, name);

    // Get the <name>-connection secret
    let postgres_user_secret = match secret_api
        .get_opt(secret_name_cnpg_postgres.as_str())
        .await?
    {
        Some(secret) => {
            debug!("Found the secret {}", secret_name_cnpg_postgres);
            secret
        }
        None => {
            debug!("Didn't find the secret {}", secret_name_cnpg_postgres);
            return Err(ConductorError::PostgresConnectionInfoNotFound);
        }
    };

    // Get the <name>-app secret
    let app_user_secret = match secret_api.get_opt(secret_name_cnpg_app.as_str()).await? {
        Some(secret) => {
            debug!("Found the secret {}", secret_name_cnpg_app);
            secret
        }
        None => {
            debug!("Didn't find the secret {}", secret_name_cnpg_app);
            return Err(ConductorError::PostgresConnectionInfoNotFound);
        }
    };

    Ok((postgres_user_secret, app_user_secret))
}

// Helper function to get the base64 values from the secret data
fn get_field_value_from_secret(
    data: &std::collections::BTreeMap<String, k8s_openapi::ByteString>,
) -> Result<(String, String), ConductorError> {
    // Get username and password from data
    let user_data = data
        .get("username")
        .ok_or(ConductorError::PostgresConnectionInfoNotFound)?;
    let byte_user =
        to_string(user_data).map_err(|_| ConductorError::ParsingPostgresConnectionError)?;
    let string_user: String =
        from_str(&byte_user).map_err(|_| ConductorError::PostgresConnectionInfoNotFound)?;

    let pw_data = data
        .get("password")
        .ok_or(ConductorError::PostgresConnectionInfoNotFound)?;
    let byte_pw = to_string(pw_data).map_err(|_| ConductorError::PostgresConnectionInfoNotFound)?;
    let string_pw: String =
        from_str(&byte_pw).map_err(|_| ConductorError::PostgresConnectionInfoNotFound)?;

    Ok((string_user, string_pw))
}

pub async fn get_pg_conn(
    client: Client,
    name: &str,
    basedomain: &str,
    spec: &CoreDBSpec,
) -> Result<types::ConnectionInfo, ConductorError> {
    let (postgres_user_secret, app_user_secret) = get_secret_for_db(client, name).await?;

    let postgres_data =
        postgres_user_secret
            .data
            .as_ref()
            .ok_or(ConductorError::SecretDataNotFound(
                "postgres_user_secret".to_string(),
            ))?;
    let app_data = app_user_secret
        .data
        .as_ref()
        .ok_or(ConductorError::SecretDataNotFound(
            "app_user_secret".to_string(),
        ))?;

    let (postgres_user, postgres_pw) = get_field_value_from_secret(postgres_data)?;
    let (app_user, app_pw) = get_field_value_from_secret(app_data)?;

    let host = format!("{name}.{basedomain}");

    // If connectionPooler is enabled, set the pooler_host
    let pooler_host = match spec.connectionPooler.enabled {
        true => {
            if spec.connectionPooler.enabled {
                Some(format!("{name}-pooler.{basedomain}"))
            } else {
                None
            }
        }
        _ => None,
    };

    // Create ConnectionInfo for the postgres user
    // The user and password are base64 encoded when passed back to the control-plane
    let postgres_conn = types::ConnectionInfo {
        host: host.clone(),
        pooler_host,
        port: 5432,
        user: postgres_user,
        password: postgres_pw,
        app_user,
        app_password: app_pw,
    };

    Ok(postgres_conn)
}

pub async fn restart_coredb(
    client: Client,
    namespace: &str,
    cluster_name: &str,
    msg_enqueued_at: DateTime<Utc>,
) -> Result<bool, ConductorError> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let restart = msg_enqueued_at
        .to_rfc3339_opts(SecondsFormat::Secs, true)
        .to_string();

    let current_coredb = coredb_api
        .get(cluster_name)
        .await
        .map_err(ConductorError::KubeError)?;
    let mut is_being_updated = false;
    match current_coredb
        .annotations()
        .get("kubectl.kubernetes.io/restartedAt")
    {
        None => {
            info!(
                "No restart annotation found on the CoreDB resource, applying for first time: {}",
                namespace
            );
            is_being_updated = true;
        }
        Some(annotation) => {
            if annotation != &restart {
                info!(
                    "Annotation found on the CoreDB resource, updating from {} to {}: {}",
                    annotation, restart, namespace
                );
                is_being_updated = true;
            }
        }
    };
    if !is_being_updated {
        info!(
            "CoreDB resource already has the correct restart annotation: {}",
            namespace
        );
        return Ok(is_being_updated);
    }

    // To restart the CNPG pod we need to annotate the Cluster resource with
    // kubectl.kubernetes.io/restartedAt: <timestamp>
    let patch_json = serde_json::json!({
        "metadata": {
            "annotations": {
                "kubectl.kubernetes.io/restartedAt": restart
            }
        }
    });

    info!("Applying `restartedAt == {restart}` to the CoreDB resource.");

    // Use the patch method to update the Cluster resource
    let params = PatchParams::default();
    let _patch = coredb_api
        .patch(cluster_name, &params, &Patch::Merge(patch_json))
        .await
        .map_err(ConductorError::KubeError)?;
    Ok(true)
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

// Generate a random cron expression based on the input string
pub fn generate_cron_expression(input: &str) -> String {
    // Create a hasher instance
    let mut hasher = DefaultHasher::new();
    // Hash the input string
    input.hash(&mut hasher);
    let hash = hasher.finish();

    // Generate hour and minute for the cron expression
    let hour = (hash % 24) as u8; // Hours: 0-23
    let minute = (hash % 60) as u8; // Minutes: 0-59

    // Construct the 5-term cron expression for a daily job
    format!("{} {} * * *", minute, hour)
}

#[cfg(test)]
mod tests {
    const DECODER: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::general_purpose::PAD,
    );

    use super::*;
    use base64::Engine;

    #[test]
    fn test_get_field_value_from_secret() {
        let mut mock_data = std::collections::BTreeMap::new();
        mock_data.insert(
            "username".to_string(),
            k8s_openapi::ByteString("mock_user".as_bytes().to_vec()),
        );
        mock_data.insert(
            "password".to_string(),
            k8s_openapi::ByteString("mock_pw".as_bytes().to_vec()),
        );

        let (user, pw) = get_field_value_from_secret(&mock_data).unwrap();

        // Decode the base64 values
        let decoded_user = DECODER.decode(user).unwrap();
        let decoded_pw = DECODER.decode(pw).unwrap();

        assert_eq!(String::from_utf8(decoded_user).unwrap(), "mock_user");
        assert_eq!(String::from_utf8(decoded_pw).unwrap(), "mock_pw");
    }

    #[test]
    fn test_valid_cron_expression() {
        let input = "test_string";
        let cron_expression = generate_cron_expression(input);
        let parts: Vec<&str> = cron_expression.split_whitespace().collect();

        assert_eq!(parts.len(), 5);
        assert_eq!(parts[2], "*");
        assert_eq!(parts[3], "*");
        assert_eq!(parts[4], "*");

        let minute: u8 = parts[0].parse().expect("Expected minute to be a number");
        let hour: u8 = parts[1].parse().expect("Expected hour to be a number");

        assert!(minute < 60, "Minute should be less than 60");
        assert!(hour < 24, "Hour should be less than 24");
    }

    #[test]
    fn test_deterministic_output() {
        let input = "consistent_input";
        let first_run = generate_cron_expression(input);
        let second_run = generate_cron_expression(input);

        assert_eq!(
            first_run, second_run,
            "Cron expressions should match for the same input"
        );
    }

    #[test]
    fn test_different_inputs_produce_different_outputs() {
        let input_one = "input_one";
        let input_two = "input_two";

        let output_one = generate_cron_expression(input_one);
        let output_two = generate_cron_expression(input_two);

        assert_ne!(
            output_one, output_two,
            "Different inputs should produce different cron expressions"
        );
    }
}
