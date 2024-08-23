use std::ops::Not;

use actix_http::Request;
use actix_web::{test, web, App, Error};
use dataplane_webserver::config;
use dataplane_webserver::routes::secrets::{
    get_secret_names_v1, get_secret_v1, update_postgres_password,
};
use serde_json::json;
use tracing_test::traced_test;

async fn create_test_app() -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let cfg = config::Config::default();
    let http_client = reqwest::Client::new();

    test::init_service(
        App::new()
            .app_data(web::Data::new(cfg))
            .app_data(web::Data::new(http_client))
            .service(
                web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(get_secret_names_v1)
                    .service(get_secret_v1)
                    .service(update_postgres_password),
            ),
    )
    .await
}

#[traced_test]
#[actix_web::test]
async fn test_get_secret_names_v1() {

    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/api/v1/orgs/org_2T7FJA0DpaNBnELVLU1IS4XzZG0/instances/inst_1696253936968_TblNOY_6/secrets")
        .to_request();

    let resp = test::call_service(&app, req).await;

    let status_code = resp.status();
    if status_code.is_success().not() {
        let bytes = test::read_body(resp).await;
        let msg = std::str::from_utf8(&bytes).unwrap();
        panic!("Request failed: {} - {msg}", status_code.as_u16())
    }

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array(), "Response body is not an array");
    assert!(
        !body.as_array().unwrap().is_empty(),
        "Response body is empty"
    );

    let secret_names: Vec<String> = body
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|secret| secret["name"].as_str().map(String::from))
        .collect();

    assert!(
        secret_names.contains(&"app-role".to_string()),
        "app-role not found in response"
    );
    assert!(
        secret_names.contains(&"readonly-role".to_string()),
        "readonly-role not found in response"
    );
    assert!(
        secret_names.contains(&"superuser-role".to_string()),
        "superuser-role not found in response"
    );
    assert!(
        secret_names.contains(&"certificate".to_string()),
        "certificate not found in response"
    );
}

#[traced_test]
#[actix_web::test]
async fn test_get_secret_v1() {
    let app = create_test_app().await;

    let req = test::TestRequest::get()
        .uri("/api/v1/orgs/org_2T7FJA0DpaNBnELVLU1IS4XzZG0/instances/inst_1696253936968_TblNOY_6/secrets/readonly-role")
        .to_request();

    let resp = test::call_service(&app, req).await;

    let status_code = resp.status();
    if status_code.is_success().not() {
        let bytes = test::read_body(resp).await;
        let msg = std::str::from_utf8(&bytes).unwrap();
        panic!("Request failed: {} - {msg}", status_code.as_u16())
    }

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object(), "Response body is not an object");
    assert!(
        body.get("username").is_some(),
        "username not found in response"
    );
    assert!(
        body.get("password").is_some(),
        "password not found in response"
    );
}
