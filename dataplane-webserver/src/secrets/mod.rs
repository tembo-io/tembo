use crate::routes::secrets::SECRETS_ALLOW_LIST;
use crate::secrets::types::AvailableSecret;
use actix_web::HttpResponse;
use k8s_openapi::ByteString;
use kube::{Api, Client};
use log::error;
use std::collections::BTreeMap;

pub mod types;

pub async fn get_secret_data_from_kubernetes(
    kubernetes_client: Client,
    namespace: String,
    requested_secret: &AvailableSecret,
) -> HttpResponse {
    let kubernetes_secret_name = requested_secret.kube_secret_name(&namespace);

    let secrets_api: Api<k8s_openapi::api::core::v1::Secret> =
        Api::namespaced(kubernetes_client, &namespace);
    let kube_secret = secrets_api.get(&kubernetes_secret_name).await;

    match kube_secret {
        Ok(secret) => {
            let mut filtered_data: BTreeMap<String, String> = BTreeMap::new();
            let secret_data = match secret.data {
                None => {
                    error!(
                        "Secret '{}' found in namespace '{}' does not have a 'data' block.",
                        kubernetes_secret_name, namespace
                    );
                    return HttpResponse::NotFound().json("Secret not found to have data block");
                }
                Some(data) => data,
            };
            for key in &requested_secret.possible_keys {
                if let Some(value) = secret_data.get(key) {
                    let value = match byte_string_to_string(value) {
                        Ok(val) => val,
                        Err(http_response) => return http_response,
                    };
                    filtered_data.insert(key.clone(), value);
                }
            }
            HttpResponse::Ok().json(filtered_data)
        }
        Err(_) => {
            error!(
                "Secret '{}' not found in namespace '{}'",
                kubernetes_secret_name, namespace
            );
            HttpResponse::NotFound().json("Secret not found")
        }
    }
}

pub fn validate_requested_secret(secret_name: &str) -> Result<&AvailableSecret, String> {
    let requested_secret = SECRETS_ALLOW_LIST
        .iter()
        .find(|&secret| &secret.name == secret_name);

    if requested_secret.is_none() {
        return Err(format!(
            "Secret '{}' not found as allowed name",
            secret_name
        ));
    }

    let secret_config = requested_secret.expect("We just checked this is not none");
    Ok(secret_config)
}

pub fn byte_string_to_string(byte_string: &ByteString) -> Result<String, HttpResponse> {
    match String::from_utf8(byte_string.0.clone()) {
        Ok(value) => Ok(value),
        Err(_) => {
            error!("Failed to convert secret value to UTF-8 string");
            Err(HttpResponse::InternalServerError()
                .json("Failed to convert secret value to UTF-8 string"))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::secrets::byte_string_to_string;
    use k8s_openapi::api::core::v1::Secret;

    #[test]
    fn test_byte_string_to_string_from_json() {
        let k8s_secret_data = r#"
        {
          "apiVersion": "v1",
          "kind": "Secret",
          "metadata": {
            "name": "my-secret"
          },
          "data": {
            "username": "dXNlcm5hbWU="
          }
        }
        "#;
        let secret: Secret = serde_json::from_str(k8s_secret_data).unwrap();
        let secret_data = secret.data.unwrap();
        let username_byte_string = secret_data.get("username").unwrap();
        let result = byte_string_to_string(username_byte_string).unwrap();
        assert_eq!(result, "username");
    }
}
