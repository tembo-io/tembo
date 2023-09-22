use crate::secrets::types::AvailableSecret;
use crate::{config, secrets};
use actix_web::{get, web, Error, HttpRequest, HttpResponse};
use lazy_static::lazy_static;
use log::warn;
use std::ops::Deref;

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
                formatter: Box::new(|instance_name| format!("{}-ca", instance_name)),
            }
        );
        secrets_allow_list
    };
}

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
#[get("")]
pub async fn get_secret_names() -> Result<HttpResponse, Error> {
    let allow_list = SECRETS_ALLOW_LIST.deref();
    Ok(HttpResponse::Ok().json(allow_list))
}

#[utoipa::path(
    context_path = "/{namespace}/secrets",
    params(
        ("namespace" = String, Path, example="org-myco-inst-prod", description = "Instance namespace"),
        ("secret_name", example="readonly-role", description = "Secret name"),
    ),
    responses(
        (status = 200, description = "Content of a secret. Available secrets and possible keys can be determined from a query to /{namespace}/secrets.", body = IndexMap<String, String>,
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

    let requested_secret = match secrets::validate_requested_secret(&secret_name) {
        Ok(secret) => secret,
        Err(e) => {
            warn!("Invalid secret requested: {}", secret_name);
            return Ok(HttpResponse::Forbidden().json(e));
        }
    };

    Ok(secrets::get_secret_data_from_kubernetes(namespace, requested_secret).await)
}
