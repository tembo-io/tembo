use crate::secrets::types::{AvailableSecret, PasswordString};
use crate::secrets::validate_requested_secret;
use crate::{config, secrets};
use actix_web::error::ErrorInternalServerError;
use actix_web::{get, patch, web, Error, HttpRequest, HttpResponse};
use base64::prelude::*;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::api::core::v1::Secret;
use kube::api::ListParams;
use kube::api::{Patch, PatchParams};
use kube::{Api, Client};
use lazy_static::lazy_static;
use log::{error, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use std::str;

lazy_static! {
    pub static ref SECRETS_ALLOW_LIST: Vec<AvailableSecret> = {
        let mut secrets_allow_list: Vec<AvailableSecret> = Vec::new();
        secrets_allow_list.push(
            AvailableSecret {
                name: "app-role".to_string(),
                possible_keys: vec!["username".to_string(), "password".to_string()],
                formatter: Box::new(|instance_name| format!("{}-app", instance_name)),
            }
        );
        secrets_allow_list.push(
            AvailableSecret {
                name: "readonly-role".to_string(),
                possible_keys: vec!["username".to_string(), "password".to_string()],
                formatter: Box::new(|instance_name| format!("{}-ro", instance_name)),
            }
        );
        secrets_allow_list.push(
            AvailableSecret {
                name: "superuser-role".to_string(),
                possible_keys: vec!["username".to_string(), "password".to_string()],
                formatter: Box::new(|instance_name| format!("{}-connection", instance_name)),
            }
        );
        secrets_allow_list.push(
            AvailableSecret {
                name: "certificate".to_string(),
                // Don't return the private key
                possible_keys: vec!["ca.crt".to_string()],
                formatter: Box::new(|instance_name| format!("{}-ca1", instance_name)),
            }
        );
        secrets_allow_list
    };
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    organizations: HashMap<String, String>,
}

/// Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets
#[utoipa::path(
    context_path = "/{namespace}/secrets",
    params(
        ("namespace" = String, Path, example="org-myco-inst-prod", description = "Instance namespace"),
    ),
    responses(
        (status = 200, description = "Map of secret names and the keys this user is authorized for", body = Vec<AvailableSecret>,
        example = json!([
            {"name":"app-role","possible_keys":["username","password"]},
            {"name":"readonly-role","possible_keys":["username","password"]},
            {"name":"superuser-role","possible_keys":["username","password"]},
            {"name":"certificate","possible_keys":["ca.crt"]}])),
        (status = 403, description = "Not authorized for query"),
    )
)]
#[deprecated]
#[get("")]
pub async fn get_secret_names() -> Result<HttpResponse, Error> {
    let allow_list = SECRETS_ALLOW_LIST.deref();
    Ok(HttpResponse::Ok().json(allow_list))
}

/// Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets/{secret_name}
#[utoipa::path(
    context_path = "/{namespace}/secrets",
    params(
        ("namespace" = String, Path, example="org-myco-inst-prod", description = "Instance namespace"),
        ("secret_name", example="readonly-role", description = "Secret name"),
    ),
    responses(
        (status = 200,
            description = "Content of a secret. Available secrets and possible keys can be determined from a query to /{namespace}/secrets.",
            body = IndexMap<String, String>,
        example = json!({ "password": "sv5uli3gR3XPbjwz", "username": "postgres" })),
        (status = 403, description = "Not authorized for query"),
    )
)]
#[deprecated]
#[get("/{secret_name}")]
pub async fn get_secret(
    _cfg: web::Data<config::Config>,
    _req: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, Error> {
    let (namespace, secret_name) = path.into_inner();

    let requested_secret = match secrets::validate_requested_secret(&secret_name) {
        Ok(secret) => secret,
        Err(e) => {
            warn!("Invalid secret requested: {}", secret_name);
            return Ok(HttpResponse::Forbidden().json(e));
        }
    };

    let kubernetes_client = match Client::try_default().await {
        Ok(client) => client,
        Err(_) => {
            error!("Failed to create Kubernetes client");
            return Ok(
                HttpResponse::InternalServerError().json("Failed to create Kubernetes client")
            );
        }
    };

    Ok(
        secrets::get_secret_data_from_kubernetes(kubernetes_client, namespace, requested_secret)
            .await,
    )
}

#[utoipa::path(
    context_path = "/api/v1/orgs/{org_id}/instances/{instance_id}",
    params(
        ("org_id" = String, Path, example="org_2T7FJA0DpaNBnELVLU1IS4XzZG0", description = "Tembo Cloud Organization ID"),
        ("instance_id" = String, Path, example="inst_1696253936968_TblNOY_6", description = "Tembo Cloud Instance ID"),
    ),
    responses(
        (status = 200, description = "Map of secret names and the keys this user is authorized for", body = Vec<AvailableSecret>,
        example = json!([
            {"name":"app-role","possible_keys":["username","password"]},
            {"name":"readonly-role","possible_keys":["username","password"]},
            {"name":"superuser-role","possible_keys":["username","password"]},
            {"name":"certificate","possible_keys":["ca.crt"]}])),
        (status = 403, description = "Not authorized for query"),
    )
)]
#[get("/secrets")]
pub async fn get_secret_names_v1() -> Result<HttpResponse, Error> {
    let allow_list = SECRETS_ALLOW_LIST.deref();
    Ok(HttpResponse::Ok().json(allow_list))
}

#[utoipa::path(
    context_path = "/api/v1/orgs/{org_id}/instances/{instance_id}",
    params(
        ("org_id" = String, Path, example="org_2T7FJA0DpaNBnELVLU1IS4XzZG0", description = "Tembo Cloud Organization ID"),
        ("instance_id" = String, Path, example="inst_1696253936968_TblNOY_6", description = "Tembo Cloud Instance ID"),
        ("secret_name", example="readonly-role", description = "Secret name"),
    ),
    responses(
        (status = 200,
            description = "Content of a secret. Available secrets and possible keys can be determined from a query to /{namespace}/secrets.",
            body = IndexMap<String, String>,
            example = json!({ "password": "sv5uli3gR3XPbjwz", "username": "postgres" })),
        (status = 403, description = "Not authorized for query"),
    )
)]
#[get("/secrets/{secret_name}")]
pub async fn get_secret_v1(
    _cfg: web::Data<config::Config>,
    _req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, Error> {
    // Requests are auth'd by org_id before entering this function

    let (org_id, instance_id, secret_name) = path.into_inner();

    get_secret_data_for_instance(&org_id, &instance_id, &secret_name).await
}

pub async fn get_secret_data_for_instance(
    org_id: &str,
    instance_id: &str,
    secret_name: &str,
) -> Result<HttpResponse, Error> {
    if !is_valid_id(&org_id) || !is_valid_id(&instance_id) {
        return Ok(HttpResponse::BadRequest()
            .json("org_id and instance_id must be alphanumeric or underscore only"));
    }

    let requested_secret = match secrets::validate_requested_secret(&secret_name) {
        Ok(secret) => secret,
        Err(e) => {
            warn!("Invalid secret requested: {}", secret_name);
            return Ok(HttpResponse::Forbidden().json(e));
        }
    };

    let kubernetes_client = match Client::try_default().await {
        Ok(client) => client,
        Err(_) => {
            error!("Failed to create Kubernetes client");
            return Ok(
                HttpResponse::InternalServerError().json("Failed to create Kubernetes client")
            );
        }
    };
    // Find namespace by labels
    let namespaces: Api<Namespace> = Api::all(kubernetes_client.clone());

    let label_selector = format!(
        "tembo.io/instance_id={},tembo.io/organization_id={}",
        instance_id, org_id
    );
    let lp = ListParams::default().labels(&label_selector);
    let ns_list = match namespaces.list(&lp).await {
        Ok(list) => list,
        Err(_) => {
            error!(
                "Failed to list namespaces with label selector: {}",
                label_selector
            );
            return Ok(HttpResponse::InternalServerError().json("Failed to list namespaces"));
        }
    };

    let namespace = match ns_list.iter().next() {
        Some(namespace) => namespace
            .metadata
            .name
            .as_ref()
            .expect("Namespaces always have names")
            .to_string(),
        None => {
            error!("No namespace found with provided labels");
            return Ok(HttpResponse::NotFound()
                .json("Instance not found for provided org_id and instance_id"));
        }
    };

    Ok(
        secrets::get_secret_data_from_kubernetes(kubernetes_client, namespace, requested_secret)
            .await,
    )
}

fn is_valid_id(s: &str) -> bool {
    let re = Regex::new(r"^[A-Za-z0-9_]+$").unwrap();
    re.is_match(s)
}

#[utoipa::path(
    context_path = "/api/v1/orgs/{org_id}/instances/{instance_id}",
    params(
        ("org_id" = String, Path, example="org_2T7FJA0DpaNBnELVLU1IS4XzZG0", description = "Tembo Cloud Organization ID"),
        ("instance_id" = String, Path, example="inst_1696253936968_TblNOY_6", description = "Tembo Cloud Instance ID"),
        ("secret_name", example="readonly-role", description = "Secret name")
    ),
    request_body = PasswordString,
    responses(
        (status = 200,
            description = "Password successfully changed."),
        (status = 403,
            description = "Not authorized for query"),
    )
)]
#[patch("/secrets/{secret_name}")]
async fn update_postgres_password(
    path: web::Path<(String, String, String)>,
    updated_password: web::Json<PasswordString>,
    _cfg: web::Data<config::Config>,
    _req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let (org_id, instance_id, secret_name) = path.into_inner();
    let password = &updated_password.password;
    let auth_header = _req
        .headers()
        .get("Authorization")
        .and_then(|hv| hv.to_str().ok())
        .and_then(|hv| hv.strip_prefix("Bearer "))
        .unwrap_or("");

    let decoding_key = DecodingKey::from_secret("".as_ref());
    let mut validation = Validation::new(Algorithm::RS256);
    validation.insecure_disable_signature_validation();

    match decode::<Claims>(auth_header, &decoding_key, &validation) {
        Ok(token_data) => match token_data.claims.organizations.get(&org_id) {
            Some(role) if role == "admin" => {
                if !is_valid_id(&org_id) || !is_valid_id(&instance_id) {
                    return Ok(HttpResponse::BadRequest()
                        .json("org_id and instance_id must be alphanumeric or underscore only"));
                }

                if password.len() < 16 {
                    return Ok(HttpResponse::BadRequest()
                        .json("Password must be at least 16 characters long"));
                }

                let encoded_password = BASE64_STANDARD.encode(password.as_bytes());
                let kubernetes_client = match Client::try_default().await {
                    Ok(client) => client,
                    Err(_) => {
                        return Ok(HttpResponse::InternalServerError()
                            .json("Failed to create Kubernetes client"))
                    }
                };

                let namespaces: Api<Namespace> = Api::all(kubernetes_client.clone());
                let label_selector = format!(
                    "tembo.io/instance_id={},tembo.io/organization_id={}",
                    instance_id, org_id
                );
                let lp = ListParams::default().labels(&label_selector);
                let ns_list = namespaces.list(&lp).await.map_err(|e| {
                    log::error!("Failed to list namespaces: {:?}", e);
                    ErrorInternalServerError("Failed to list namespaces")
                })?;

                let namespace = ns_list
                    .iter()
                    .next()
                    .ok_or_else(|| {
                        log::error!("No namespace found with provided labels");
                        ErrorInternalServerError(
                            "Instance not found for provided org_id and instance_id",
                        )
                    })?
                    .metadata
                    .name
                    .as_ref()
                    .unwrap()
                    .clone();

                let requested_secret = match validate_requested_secret(&secret_name) {
                    Ok(secret) if secret.name.ends_with("-role") => secret,
                    Ok(_) => return Ok(HttpResponse::Forbidden().json("Password can only be patched by roles. Ex: superuser-role, readonly-role, app-role")),
                    Err(_) => return Ok(HttpResponse::Forbidden().json("Invalid secret name. Please find valid secrets under /api/v1/orgs/{org_id}/instances/{instance_id}/secrets")),
                };
                let secret_name_to_patch = (requested_secret.formatter)(&namespace);

                let secrets_api: Api<Secret> =
                    Api::namespaced(kubernetes_client, &namespace.clone());
                let patch_data = serde_json::json!({
                "data": {
                    "password": encoded_password
                }
                });
                let params = PatchParams::default();
                let patch_result = secrets_api
                    .patch(&secret_name_to_patch, &params, &Patch::Merge(&patch_data))
                    .await;

                if let Err(e) = patch_result {
                    log::error!("Failed to update secret: {:?}", e);
                    return Err(ErrorInternalServerError("Failed to update secret"));
                }

                Ok(HttpResponse::Ok().json("Password updated successfully"))
            }
            _ => {
                return Err(actix_web::error::ErrorForbidden("Not authorized"));
            }
        },
        Err(err) => {
            eprintln!("Error decoding token: {:?}", err);
            return Err(actix_web::error::ErrorBadRequest("Invalid token"));
        }
    }
}
