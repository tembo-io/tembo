use azure_identity::AzureCliCredential;
use azure_mgmt_authorization;
use azure_mgmt_authorization::models::RoleAssignmentProperties;
use azure_mgmt_msi::models::{Identity, TrackedResource};
use std::sync::Arc;

#[tokio::main]
pub async fn create_uami() -> Result<(), Box<dyn std::error::Error>> {
    let credential = Arc::new(AzureCliCredential::new());
    let subscription_id = AzureCliCredential::get_subscription().await?;
    let resource_group_name = "ian".to_string();
    let uami_name = "test-uami".to_string();
    let msi_client = azure_mgmt_msi::Client::builder(credential.clone()).build()?;

    // Create User Assigned Managed Identity
    let uami_params = Identity {
        tracked_resource: TrackedResource {
            resource: Default::default(),
            tags: None,
            location: "eastus".to_string(),
        },
        properties: None,
    };

    let uami_created = msi_client
        .user_assigned_identities_client()
        .create_or_update(
            subscription_id.clone(),
            resource_group_name,
            uami_name,
            uami_params,
        )
        .await?;
    println!("UAMI created: {uami_created:#?}");

    let uami_id = uami_created.properties.unwrap().principal_id.unwrap();

    // Create Role Assignment for the UAMI created above
    let role_name = "Contributor";
    let role_assignment_name = "00000000-0000-0000-0000-000000000000".to_string();
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

    let role_assignment_created = role_assignment_client
        .role_assignments_client()
        .create(scope, role_assignment_name, role_assignment_params)
        .await?;
    println!("Role Assignment created: {role_assignment_created:#?}");

    Ok(())
}
