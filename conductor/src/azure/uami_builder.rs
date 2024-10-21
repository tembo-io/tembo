use crate::azure::azure_error;
use azure_core::auth::TokenCredential;
use azure_core::error::Error as AzureSDKError;
use azure_error::AzureError;
use azure_identity::TokenCredentialOptions;
use azure_identity::WorkloadIdentityCredential;
use azure_mgmt_authorization;
use azure_mgmt_authorization::models::{RoleAssignment, RoleAssignmentProperties};
use azure_mgmt_msi::models::{
    FederatedIdentityCredential, FederatedIdentityCredentialProperties, Identity, TrackedResource,
};
use futures::StreamExt;
use log::info;
use std::sync::Arc;

// Get credentials from workload identity
pub async fn get_credentials() -> Result<Arc<dyn TokenCredential>, AzureError> {
    let options: TokenCredentialOptions = Default::default();
    let credential = WorkloadIdentityCredential::create(options)?;
    Ok(Arc::new(credential))
}

// Create User Assigned Managed Identity
pub async fn create_uami(
    resource_group_prefix: &str,
    subscription_id: &str,
    uami_name: &str,
    region: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<Identity, AzureError> {
    let resource_group = format!("{resource_group_prefix}-storage-rg");
    let msi_client = azure_mgmt_msi::Client::builder(credentials).build()?;

    // Set parameters for User Assigned Managed Identity
    let uami_params = Identity {
        tracked_resource: TrackedResource {
            resource: Default::default(),
            tags: None,
            location: region.to_string(),
        },
        properties: None,
    };

    // Create User Assigned Managed Identity
    let uami_created = msi_client
        .user_assigned_identities_client()
        .create_or_update(subscription_id, resource_group, uami_name, uami_params)
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
    Err(AzureError::from(AzureSDKError::new(
        azure_core::error::ErrorKind::Other,
        format!("Role definition {} not found", role_name),
    )))
}

// Get storage account ID
pub async fn get_storage_account_id(
    subscription_id: &str,
    resource_group_prefix: &str,
    storage_account_name: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<String, AzureError> {
    let resource_group = format!("{resource_group_prefix}-storage-rg");
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

// Check if role assignment exists
pub async fn role_assignment_exists(
    subscription_id: &str,
    _storage_account_id: &str,
    uami_id: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<bool, AzureError> {
    let role_assignment_client =
        azure_mgmt_authorization::Client::builder(credentials.clone()).build()?;

    let role_definition = get_role_definition_id(
        subscription_id,
        "Storage Blob Data Contributor",
        credentials.clone(),
    )
    .await?;

    let role_assignment_list = role_assignment_client
        .role_assignments_client()
        .list_for_subscription(subscription_id);
    let mut role_assignment_stream = role_assignment_list.into_stream();
    while let Some(role_assignment_page) = role_assignment_stream.next().await {
        let role_assignment_page = role_assignment_page?;
        for item in role_assignment_page.value {
            if item.properties.clone().unwrap().role_definition_id == role_definition
                && item.properties.unwrap().principal_id == uami_id
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

// Create Role Assignment for UAMI
pub async fn create_role_assignment(
    subscription_id: &str,
    resource_group_prefix: &str,
    storage_account_name: &str,
    uami_id: &str,
    uami_principal_id: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<RoleAssignment, AzureError> {
    let resource_group = format!("{resource_group_prefix}-storage-rg");
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
        &resource_group,
        storage_account_name,
        credentials.clone(),
    )
    .await?;

    // Check if role assignment already exists
    info!("Checking if role assignment exists");
    if role_assignment_exists(
        subscription_id,
        &storage_account_id,
        uami_principal_id,
        credentials,
    )
    .await?
    {
        info!("Role assignment already exists, skipping creation");
        return Ok(RoleAssignment {
            id: None,
            name: None,
            type_: None,
            properties: None,
        });
    }

    // Set parameters for Role Assignment
    let role_assignment_params = azure_mgmt_authorization::models::RoleAssignmentCreateParameters {
        properties: RoleAssignmentProperties {
            scope: None,
            role_definition_id: role_definition,
            principal_id: uami_id.to_string(),
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

// Get OIDC Issuer URL from AKS cluster using rest API. This is necessary because the azure_mgmt_containerservice
// crate is no longer being built: https://github.com/Azure/azure-sdk-for-rust/pull/1243
pub async fn get_cluster_issuer(
    subscription_id: &str,
    resource_group_prefix: &str,
    cluster_name: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<String, AzureError> {
    let resource_group = format!("{resource_group_prefix}-aks-rg");
    let client = reqwest::Client::new();
    let url = format!(
        "https://management.azure.com/subscriptions/{subscription_id}/resourceGroups/{resource_group}/providers/Microsoft.ContainerService/managedClusters/{cluster_name}?api-version=2024-08-01");
    let scopes: &[&str] = &["https://management.azure.com/.default"];

    let response = client
        .get(&url)
        .header(
            "Authorization",
            format!(
                "Bearer {}",
                credentials.get_token(scopes).await?.token.secret()
            ),
        )
        .send()
        .await?;

    let response_json = response.json::<serde_json::Value>().await?;
    let issuer_url = response_json["properties"]["oidcIssuerProfile"]["issuerURL"]
        .as_str()
        .unwrap();
    Ok(issuer_url.to_string())
}

// Create Federated Identity Credentials for the UAMI
pub async fn create_federated_identity_credentials(
    subscription_id: &str,
    resource_group_prefix: &str,
    instance_name: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<FederatedIdentityCredential, AzureError> {
    let resource_group = format!("{resource_group_prefix}-storage-rg");
    let uami_name = instance_name;
    let federated_identity_client = azure_mgmt_msi::Client::builder(credentials.clone()).build()?;
    let cluster_issuer = get_cluster_issuer(
        subscription_id,
        &resource_group,
        "aks-cdb-plat-eus2-sandbox-aks-data-1", // TODO(ianstanton) do not hard-code cluster_name
        credentials.clone(),
    )
    .await?;

    // Set parameters for Federated Identity Credentials
    let federated_identity_params = FederatedIdentityCredential {
        proxy_resource: Default::default(),
        properties: Some(FederatedIdentityCredentialProperties {
            issuer: cluster_issuer,
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
            uami_name,
            instance_name,
            federated_identity_params,
        )
        .await?;
    Ok(federated_identity_created)
}

// Delete User Assigned Managed Identity
pub async fn delete_uami(
    subscription_id: &str,
    resource_group_prefix: &str,
    uami_name: &str,
    credentials: Arc<dyn TokenCredential>,
) -> Result<(), AzureError> {
    let resource_group = format!("{resource_group_prefix}-storage-rg");
    let msi_client = azure_mgmt_msi::Client::builder(credentials).build()?;
    msi_client
        .user_assigned_identities_client()
        .delete(subscription_id, resource_group, uami_name)
        .send()
        .await?;
    Ok(())
}
