use azure_core::auth::TokenCredential;
use azure_core::error::Error as AzureError;
use azure_identity::WorkloadIdentityCredential;
use azure_identity::{AzureCliCredential, TokenCredentialOptions};
use azure_mgmt_authorization;
use azure_mgmt_authorization::models::{RoleAssignment, RoleAssignmentProperties, RoleDefinition};
use azure_mgmt_msi::models::{
    FederatedIdentityCredential, FederatedIdentityCredentialProperties, Identity, TrackedResource,
};
use azure_mgmt_msi::user_assigned_identities::delete::Response;
use futures::StreamExt;
use schemars::_private::NoSerialize;
use std::sync::Arc;

// Get credentials from workload identity
pub async fn get_credentials() -> Result<Arc<dyn TokenCredential>, AzureError> {
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
) -> Result<Identity, AzureError> {
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

// Get role definition ID
pub async fn get_role_definition_id(
    subscription_id: &str,
    role_name: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<String, AzureError> {
    let role_definition_client = azure_mgmt_authorization::Client::builder(credentials).build()?;
    let scope = format!("/subscriptions/{subscription_id}");
    // Get role definition for role name
    let role_definition = role_definition_client.role_definitions_client().list(scope);
    let mut role_definition_stream = role_definition.into_stream();
    while let Some(role_definition_page) = role_definition_stream.next().await {
        let role_definition_page = role_definition_page?;
        for item in role_definition_page.value {
            if item.properties.unwrap().role_name == Some(role_name.to_string()) {
                return Ok(item.id.unwrap());
            }
        }
    }
    // Return error if not found
    Err(AzureError::new(
        azure_core::error::ErrorKind::Other,
        format!("Role definition {} not found", role_name),
    ))
}

// Get storage account ID
pub async fn get_storage_account_id(
    subscription_id: &str,
    resource_group: &str,
    storage_account_name: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<String, AzureError> {
    let storage_client = azure_mgmt_storage::Client::builder(credentials).build()?;
    let storage_account_list = storage_client
        .storage_accounts_client()
        .list_by_resource_group(resource_group, subscription_id);
    let mut storage_account_stream = storage_account_list.into_stream();
    let mut storage_account = None;
    while let Some(storage_account_page) = storage_account_stream.next().await {
        let storage_account_page = storage_account_page?;
        for item in storage_account_page.value {
            if item.tracked_resource.resource.name == Some(storage_account_name.to_string()) {
                storage_account = Some(item);
                break;
            }
        }
        if storage_account.is_some() {
            break;
        }
    }
    Ok(storage_account
        .unwrap()
        .tracked_resource
        .resource
        .id
        .unwrap())
}

// Create Role Assignment for UAMI
pub async fn create_role_assignment(
    subscription_id: &str,
    resource_group: &str,
    storage_account_name: &str,
    uami_id: String,
    credentials: Arc<dyn TokenCredential>,
) -> Result<RoleAssignment, AzureError> {
    let role_assignment_name = uuid::Uuid::new_v4().to_string();
    let role_assignment_client =
        azure_mgmt_authorization::Client::builder(credentials.clone()).build()?;

    let role_definition = get_role_definition_id(
        subscription_id,
        "Storage Blob Data Contributor",
        credentials.clone(),
    )
    .await?;

    // TODO(ianstanton) Set conditions for Role Assignment. These should allow for read / write
    //  to the instance's directory in the blob

    let storage_account_id = get_storage_account_id(
        subscription_id,
        resource_group,
        storage_account_name,
        credentials,
    )
    .await?;

    // Set parameters for Role Assignment
    let role_assignment_params = azure_mgmt_authorization::models::RoleAssignmentCreateParameters {
        properties: RoleAssignmentProperties {
            scope: None,
            role_definition_id: role_definition,
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

    // Create Role Assignment. Scope should be storage account ID
    let role_assignment_created = role_assignment_client
        .role_assignments_client()
        .create(
            storage_account_id,
            role_assignment_name,
            role_assignment_params,
        )
        .await?;
    Ok(role_assignment_created)
}

// Create Federated Identity Credentials for the UAMI
pub async fn create_federated_identity_credentials(
    subscription_id: &str,
    resource_group: String,
    instance_name: String,
    credentials: Arc<dyn TokenCredential>,
    region: String,
) -> Result<FederatedIdentityCredential, AzureError> {
    let federated_identity_client = azure_mgmt_msi::Client::builder(credentials).build()?;

    // TODO(ianstanton)
    //  Get cluster issuer with something similar to this az command:
    //  export AKS_OIDC_ISSUER="$(az aks show --name "${CLUSTER_NAME}" --resource-group "${RESOURCE_GROUP}" --query "oidcIssuerProfile.issuerUrl" --output tsv)"
    let cluster_issuer = "https://<region>.oic.prod-aks.azure.com/<tenant_id>/<client_id>/";

    // Set parameters for Federated Identity Credentials
    let federated_identity_params = FederatedIdentityCredential {
        proxy_resource: Default::default(),
        properties: Some(FederatedIdentityCredentialProperties {
            issuer: cluster_issuer.to_string(),
            subject: format!("system:serviceaccount:{instance_name}:{instance_name}"),
            audiences: vec!["api://AzureADTokenExchange".to_string()],
        }),
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

// Delete User Assigned Managed Identity
pub async fn delete_uami(
    subscription_id: &str,
    resource_group: String,
    instance_name: String,
    credentials: Arc<dyn TokenCredential>,
) -> Result<(), AzureError> {
    let msi_client = azure_mgmt_msi::Client::builder(credentials).build()?;
    msi_client
        .user_assigned_identities_client()
        .delete(subscription_id, resource_group, instance_name)
        .send()
        .await?;
    Ok(())
}
