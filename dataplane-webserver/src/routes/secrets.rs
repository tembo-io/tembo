use crate::secrets::types::AvailableSecret;
use crate::{config, secrets};
use actix_web::{get, web, Error, HttpRequest, HttpResponse};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::ListParams;
use kube::{Api, Client};
use lazy_static::lazy_static;
use log::{error, warn};
use regex::Regex;
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
                formatter: Box::new(|instance_name| format!("{}-ca1", instance_name)),
            }
        );
        secrets_allow_list
    };
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
