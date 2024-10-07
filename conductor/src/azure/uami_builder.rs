use azure_identity::WorkloadIdentityCredential;
use azure_identity::{AzureCliCredential, TokenCredentialOptions};
use azure_mgmt_authorization;
use azure_mgmt_authorization::models::{RoleAssignment, RoleAssignmentProperties};
use azure_mgmt_msi::models::{Identity, TrackedResource};
use std::sync::Arc;

// Get credentials from workload identity
pub async fn get_credentials() -> Result<WorkloadIdentityCredential, Box<dyn std::error::Error>> {
    let options: TokenCredentialOptions = Default::default();
    let credential = WorkloadIdentityCredential::create(options)?;
    Ok(credential)
}

// Create User Assigned Managed Identity
pub async fn create_uami(
    resource_group: String,
    instance_name: String,
    region: String,
) -> Result<Identity, Box<dyn std::error::Error>> {
    let credential = Arc::new(AzureCliCredential::new());
    let subscription_id = AzureCliCredential::get_subscription().await?;
    let resource_group_name = resource_group;
    let uami_name = instance_name;
    let msi_client = azure_mgmt_msi::Client::builder(credential.clone()).build()?;

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
            resource_group_name,
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
) -> Result<RoleAssignment, Box<dyn std::error::Error>> {
    let credential = AzureCliCredential::new();
    let role_assignment_name = "00000000-0000-0000-0000-000000000000".to_string();

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

    let role_assignment_client = azure_mgmt_authorization::Client::builder(credential).build()?;
    let scope = format!("/subscriptions/{subscription_id}");

    // Create Role Assignment
    let role_assignment_created = role_assignment_client
        .role_assignments_client()
        .create(scope, role_assignment_name, role_assignment_params)
        .await?;
    Ok(role_assignment_created)
}
