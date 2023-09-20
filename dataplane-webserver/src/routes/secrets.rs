use crate::config;
use actix_web::{get, web, Error, HttpRequest, HttpResponse};
use lazy_static::lazy_static;
use log::error;
use std::collections::BTreeMap;

use k8s_openapi::ByteString;
use kube::{Api, Client};

type SecretNameFormatter = Box<dyn Fn(&str) -> String + Send + Sync>;

struct Secret {
    name: String,
    // For some secrets, we only return some of the keys
    allowed_keys: Vec<String>,
    // All secrets need a string formatting function
    formatter: SecretNameFormatter,
}

impl Secret {
    fn kube_secret_name(&self, instance_name: &str) -> String {
        (self.formatter)(instance_name)
    }
}

lazy_static! {
    static ref SECRETS_ALLOW_LIST: Vec<Secret> = {
        let mut secrets_allow_list: Vec<Secret> = Vec::new();
        secrets_allow_list.push(
            Secret {
                name: "app-role".to_string(),
                allowed_keys: vec!["username".to_string(), "password".to_string()],
                formatter: Box::new(|instance_name| format!("{}-app", instance_name)),
            }
        );
        secrets_allow_list.push(
            Secret {
                name: "readonly-role".to_string(),
                allowed_keys: vec!["username".to_string(), "password".to_string()],
                formatter: Box::new(|instance_name| format!("{}-ro", instance_name)),
            }
        );
        secrets_allow_list.push(
            Secret {
                name: "superuser-role".to_string(),
                allowed_keys: vec!["username".to_string(), "password".to_string()],
                formatter: Box::new(|instance_name| format!("{}-connection", instance_name)),
            }
        );
        secrets_allow_list.push(
            Secret {
                name: "certificate".to_string(),
                // Don't return the private key
                allowed_keys: vec!["ca.crt".to_string()],
                formatter: Box::new(|instance_name| format!("{}-ca", instance_name)),
            }
        );
        secrets_allow_list
    };
}

#[utoipa::path(
    context_path = "/{namespace}/secrets",
    params(
        ("namespace", example="org-myco-inst-prod", description = "Instance namespace"),
    ),
    responses(
        (status = 200, description = "Map of secret names and the keys this user is authorized for", body = Value,
        example = json!({"app-role": ["username", "password"], "certificate": ["ca.crt"]})),
        (status = 403, description = "Not authorized for query"),
    )
)]
#[get("")]
pub async fn get_secret_names() -> Result<HttpResponse, Error> {
    let mut allowed_secrets_with_keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for secret in SECRETS_ALLOW_LIST.iter() {
        allowed_secrets_with_keys.insert(secret.name.clone(), secret.allowed_keys.clone());
    }
    Ok(HttpResponse::Ok().json(allowed_secrets_with_keys))
}

#[utoipa::path(
    context_path = "/{namespace}/secrets",
    params(
        ("namespace", example="org-myco-inst-prod", description = "Instance namespace"),
        ("secret_name", example="readonly-role", description = "Secret name"),
    ),
    responses(
        (status = 200, description = "Secret content in JSON", body = Value,
        example = json!({ "password": "sv5uli3gR3XPbjwz", "username": "postgres" })),
        (status = 403, description = "Not authorized for query"),
    )
)]
#[get("/{secret_name}")]
pub async fn get_secret(
    _cfg: web::Data<config::Config>,
    _req: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, Error> {
    let (namespace, secret_name) = path.into_inner();

    // Find the appropriate Secret configuration
    let secret_config = SECRETS_ALLOW_LIST
        .iter()
        .find(|&secret| secret.name == secret_name);

    if secret_config.is_none() {
        return Ok(HttpResponse::NotFound().json(format!(
            "Secret '{}' not found as allowed name",
            secret_name
        )));
    }
    let secret_config = secret_config.expect("We just checked this is not none");
    let kubernetes_secret_name = secret_config.kube_secret_name(&namespace);

    let kubernetes_client = match Client::try_default().await {
        Ok(client) => client,
        Err(_) => {
            error!("Failed to create Kubernetes client");
            return Ok(
                HttpResponse::InternalServerError().json("Failed to create Kubernetes client")
            );
        }
    };

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
                    return Ok(HttpResponse::NotFound().json(format!(
                        "Secret '{}' not found to have data block",
                        secret_name
                    )));
                }
                Some(data) => data,
            };
            for key in &secret_config.allowed_keys {
                if let Some(value) = secret_data.get(key) {
                    let value = match byte_string_to_string(value) {
                        Ok(value) => value,
                        Err(response) => return Ok(response),
                    };
                    filtered_data.insert(key.clone(), value);
                }
            }
            Ok(HttpResponse::Ok().json(filtered_data))
        }
        Err(_) => {
            error!(
                "Secret '{}' not found in namespace '{}'",
                kubernetes_secret_name, namespace
            );
            Ok(HttpResponse::NotFound().json(format!("Secret '{}' not found", secret_name)))
        }
    }
}

fn byte_string_to_string(byte_string: &ByteString) -> Result<String, HttpResponse> {
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
    use crate::routes::secrets::byte_string_to_string;
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
