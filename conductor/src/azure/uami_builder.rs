use azure_identity::WorkloadIdentityCredential;
use azure_core::auth::TokenCredential;
use reqwest::Client;
use reqwest::Url;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = "your_tenant_id".to_string();
    let client_id = "your_client_id".to_string();
    let subscription_id = "your_subscription_id";
    let resource_group_name = "your_resource_group_name";

    let http_client = Arc::new(reqwest::Client::new());
    let authority_host = Url::parse("https://login.microsoftonline.com/")?;

    let scopes = vec!["https://management.azure.com/.default"];
    
    let credential = WorkloadIdentityCredential::new(http_client, authority_host, tenant_id, client_id, None);
    let token = credential.get_token(&scopes).await?;


    let client = Client::new();
    let base_url = format!("https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.ManagedIdentity/userAssignedIdentities", subscription_id, resource_group_name);

    // Create a new User Assigned Managed Identity
    let identity_name = "my-new-identity";
    let url = format!("{}/{}", base_url, identity_name);
    let body = json!({
        "location": "eastus",
    });
    let headers = {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    };

    client.put(url)
        .headers(headers)
        .json(&body)
        .send()
        .await?
        .text()
        .await?;

    println!("Created User Assigned Managed Identity: {}", identity_name);

    // Delete the User Assigned Managed Identity
    let url = format!("{}/{}", base_url, identity_name);
    client.delete(url)
        .headers(headers)
        .send()
        .await?
        .text()
        .await?;

    println!("Deleted User Assigned Managed Identity: {}", identity_name);

    Ok(())
}