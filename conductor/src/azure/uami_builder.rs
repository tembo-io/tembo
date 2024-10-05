use azure_identity::AzureCliCredential;
use azure_mgmt_msi::models::{Identity, TrackedResource};
use std::sync::Arc;

#[tokio::main]
pub async fn create_uami() -> Result<(), Box<dyn std::error::Error>> {
    let credential = Arc::new(AzureCliCredential::new());
    let subscription_id = AzureCliCredential::get_subscription().await?;
    let resource_group_name = "ian".to_string();
    let uami_name = "test-uami".to_string();
    let client = azure_mgmt_msi::Client::builder(credential).build()?;

    let uami_params = Identity {
        tracked_resource: TrackedResource {
            resource: Default::default(),
            tags: None,
            location: "eastus".to_string(),
        },
        properties: None,
    };

    let uami_created = client
        .user_assigned_identities_client()
        .create_or_update(subscription_id, resource_group_name, uami_name, uami_params)
        .await?;
    println!("UAMI created: {uami_created:#?}");
    Ok(())
}
