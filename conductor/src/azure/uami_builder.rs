use azure_identity::ClientSecretCredential;
use azure_identity::oauth2;
use reqwest::Client;
use reqwest::Url;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Replace with your Azure credentials
    let tenant_id = "your_tenant_id";
    let client_id = oauth2::ClientId::new("your_client_id");
    let client_secret = Some(oauth2::ClientSecret::new("your_client_secret"));
    let subscription_id = "your_subscription_id";
    let resource_group_name = "your_resource_group_name";

    let http_client = Arc::new(reqwest::Client::new());
    let authority_host = Url::parse("https://login.microsoftonline.com/")?;

    let credential = ClientSecretCredential {
        http_client,
        authority_host,
        tenant_id: tenant_id.to_string(),
        client_id,
        client_secret,
        cache: None, // or provide a custom TokenCache implementation
    };

    let token = credential.get_token("https://management.azure.com/").await?.token.secret();

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