use std::sync::Arc;
use actix_web::{test, web, App};
use dataplane_webserver::config;
use dataplane_webserver::routes::secrets::get_secret_v1;
use k8s_openapi::api::core::v1::Namespace;
use kube::Api;
use mockall::predicate::*;
use mockall::mock;

mock! {
    pub K8sClient {
        async fn list_namespaces(&self) -> Result<Vec<Namespace>, kube::Error>;
        async fn get_secret(&self, namespace: String, secret_name: String) -> Result<serde_json::Value, kube::Error>;
    }
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
    // Mock K8sClient
    let mut mock_client = MockK8sClient::new();
    mock_client.expect_list_namespaces()
        .returning(|| {
            println!("Mock list_namespaces called");
            Ok(vec![Namespace {
                metadata: kube::api::ObjectMeta {
                    name: Some("test-namespace".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }])
        });
    mock_client.expect_get_secret()
        .returning(|namespace, secret_name| {
            println!("Mock get_secret called with namespace: {}, secret_name: {}", namespace, secret_name);
            Ok(serde_json::json!({
                "username": "test_user",
                "password": "test_password"
            }))
        });

    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(config::Config::default()))
            .app_data(web::Data::new(Arc::new(mock_client) as Arc<MockK8sClient>))
            .service(
                web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(get_secret_v1)
            )
    ).await;

    // Make request
    let req = test::TestRequest::get()
        .uri("/api/v1/orgs/org_2T7FJA0DpaNBnELVLU1IS4XzZG0/instances/inst_1696253936968_TblNOY_6/secrets/app-role")
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Print response details
    println!("Response status: {:?}", resp.status());
    println!("Response headers: {:?}", resp.headers());
    
    let body = test::read_body(resp).await;
    println!("Response body: {:?}", String::from_utf8_lossy(&body));
    let status_code = resp.status();
    if status_code.is_success().not() {
        let bytes = test::read_body(resp).await;
        let msg = std::str::from_utf8(&bytes).unwrap();
        panic!("Request failed: {} - {msg}", status_code.as_u16())
    }

    // Assert response
    //assert!(resp.status().is_success(), "Response status is not successful: {:?}", resp.status());

    let body: serde_json::Value = serde_json::from_slice(&body).expect("Failed to parse JSON");
    assert!(body.is_object(), "Response body is not an object");
    assert_eq!(body, serde_json::json!({
        "username": "test_user",
        "password": "test_password"
    }), "Unexpected response body");
}
    assert!(
        body.get("username").is_some(),
        "username not found in response"
    );
    assert!(
        body.get("password").is_some(),
        "password not found in response"
    );
}
