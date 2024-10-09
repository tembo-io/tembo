use azure_core::auth::TokenCredential;
use azure_identity::WorkloadIdentityCredential;
use azure_identity::{AzureCliCredential, TokenCredentialOptions};
use azure_mgmt_authorization;
use azure_mgmt_authorization::models::{RoleAssignment, RoleAssignmentProperties};
use azure_mgmt_msi::models::{FederatedIdentityCredential, Identity, TrackedResource};
use std::sync::Arc;

// Get credentials from workload identity
pub async fn get_credentials() -> Result<Arc<dyn TokenCredential>, Box<dyn std::error::Error>> {
    let options: TokenCredentialOptions = Default::default();
    let credential = WorkloadIdentityCredential::create(options)?;
    Ok(Arc::new(credential))
}

// Create User Assigned Managed Identity
pub async fn create_uami(
    resource_group: String,
    subscription_id: String,
    instance_name: String,
    region: String,
    credentials: Arc<dyn TokenCredential>,
) -> Result<Identity, Box<dyn std::error::Error>> {
    let uami_name = instance_name;
    let msi_client = azure_mgmt_msi::Client::builder(credentials).build()?;

    // Set parameters for User Assigned Managed Identity
    let uami_params = Identity {
        tracked_resource: TrackedResource {
            resource: Default::default(),
            tags: None,
            location: region,
        },
        properties: None,
    };

    // Create User Assigned Managed Identity
    let uami_created = msi_client
        .user_assigned_identities_client()
        .create_or_update(
            subscription_id.clone(),
            resource_group,
            uami_name,
            uami_params,
        )
        .await?;
    Ok(uami_created)
}

// Create Role Assignment for UAMI
pub async fn create_role_assignment(
    subscription_id: &str,
    uami_id: String,
    credentials: Arc<dyn TokenCredential>,
) -> Result<RoleAssignment, Box<dyn std::error::Error>> {
    let role_assignment_name = "00000000-0000-0000-0000-000000000000".to_string();
    let role_assignment_client = azure_mgmt_authorization::Client::builder(credentials).build()?;
    let scope = format!("/subscriptions/{subscription_id}");

    // Set parameters for Role Assignment
    let role_assignment_params = azure_mgmt_authorization::models::RoleAssignmentCreateParameters {
        properties: RoleAssignmentProperties {
            scope: None,
            role_definition_id: "ba92f5b4-2d11-453d-a403-e96b0029c9fe".to_string(),
            principal_id: uami_id,
            principal_type: None,
            description: None,
            condition: None,
            condition_version: None,
            created_on: None,
            updated_on: None,
            created_by: None,
            updated_by: None,
            delegated_managed_identity_resource_id: None,
        },
    };

    // Create Role Assignment
    let role_assignment_created = role_assignment_client
        .role_assignments_client()
        .create(scope, role_assignment_name, role_assignment_params)
        .await?;
    Ok(role_assignment_created)
}

// Create Federated Identity Credentials for the UAMI
pub async fn create_federated_identity_credentials(
    subscription_id: &str,
    resource_group: String,
    instance_name: String,
    credentials: Arc<dyn TokenCredential>,
) -> Result<FederatedIdentityCredential, Box<dyn std::error::Error>> {
    let federated_identity_client = azure_mgmt_msi::Client::builder(credentials).build()?;

    // Set parameters for Federated Identity Credentials
    let federated_identity_params = FederatedIdentityCredential {
        proxy_resource: Default::default(),
        properties: None,
    };

    // Create Federated Identity Credentials
    let federated_identity_created = federated_identity_client
        .federated_identity_credentials_client()
        .create_or_update(
            subscription_id,
            resource_group,
            &instance_name,
            &instance_name,
            federated_identity_params,
        )
        .await?;
    Ok(federated_identity_created)
}
