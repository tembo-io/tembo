mod util;

use actix_web::{http::header, http::StatusCode, test};
use rand::prelude::*;
use sqlx::Row;
use util::common;

use env_logger;
use gateway::config::Config;
use gateway::db::{self, connect};

#[ignore]
#[actix_web::test]
async fn test_probes() {
    let app = common::get_test_app(false).await;

    let req = test::TestRequest::get().uri("/ready").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let req = test::TestRequest::get().uri("/lively").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let req = test::TestRequest::get().uri("/notapath").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[ignore]
#[actix_web::test]
async fn test_logging() {
    let config = Config::new().await;

    let app = common::get_test_app(false).await;

    let mut rng = rand::thread_rng();
    let rnd = rng.gen_range(0..100000);
    let instance = format!("MY-TEST-INSTANCE-{}", rnd);
    let model = "facebook/opt-125m";
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "San Francisco is a..."}]
    });
    let req = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("X-TEMBO-ORG", "MY-TEST-ORG"))
        .insert_header(("X-TEMBO-INSTANCE", instance.clone()))
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload(payload.to_string())
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    // TOOD: parse response
    let body: serde_json::Value = test::read_body_json(resp).await;

    let choices = body.get("choices").unwrap().as_array().unwrap();
    assert_eq!(choices.len(), 1);
    choices.get(0).unwrap();

    let conn = connect(&config.pg_conn_str, 2)
        .await
        .expect("Failed to connect to database");

    let rows = sqlx::query("SELECT * FROM inference.requests WHERE instance_id = $1")
        .bind(&instance)
        .fetch_all(&conn)
        .await
        .expect("Failed to fetch log");

    assert_eq!(rows.len(), 1);

    let row = rows.get(0).unwrap();
    assert_eq!(row.get::<String, &str>("instance_id"), instance);
    assert_eq!(row.get::<String, &str>("organization_id"), "MY-TEST-ORG");
    assert_eq!(row.get::<String, &str>("model"), "facebook/opt-125m");
}

#[ignore]
#[actix_web::test]
async fn test_authorization() {
    env_logger::init();

    let mut rng = rand::thread_rng();
    let rnd = rng.gen_range(0..100000);
    let org_id = format!("org_{rnd}");

    std::env::set_var("ORG_AUTH_CACHE_REFRESH_INTERVAL_SEC", "1");
    let app = common::get_test_app(true).await;

    let model = "facebook/opt-125m";
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "San Francisco is a..."}]
    });

    let req = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("X-TEMBO-ORG", org_id.clone()))
        .insert_header(("X-TEMBO-INSTANCE", "test-instance"))
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload(payload.to_string())
        .to_request();
    let resp = test::call_service(&app, req).await;
    // this should fail because org_id is not validated
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // set the org_id to validated
    let cfg = Config::new().await;
    let dbclient = db::connect(&cfg.pg_conn_str, 1)
        .await
        .expect("Failed to connect to database");
    sqlx::query("INSERT INTO inference.org_validation (org_id, valid) VALUES ($1, $2)")
        .bind(&org_id)
        .bind(true)
        .execute(&dbclient)
        .await
        .expect("Failed to insert org status");

    // call again after org is validated
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    let req = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("X-TEMBO-ORG", org_id.clone()))
        .insert_header(("X-TEMBO-INSTANCE", "test-instance"))
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload(payload.to_string())
        .to_request();

    println!("org_id: {}", org_id);

    let resp = test::call_service(&app, req).await;
    // validated org must succeed
    println!("{:?}", resp);
    assert!(resp.status().is_success());
}

#[ignore]
#[actix_web::test]
async fn test_unavailable_model() {
    let app = common::get_test_app(false).await;

    let mut rng = rand::thread_rng();
    let rnd = rng.gen_range(0..100000);
    let instance = format!("MY-TEST-INSTANCE-{}", rnd);
    let model = "random/not-a-real-model";
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "the quick brown fox..."}]
    });
    let req = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("X-TEMBO-ORG", "MY-TEST-ORG"))
        .insert_header(("X-TEMBO-INSTANCE", instance.clone()))
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload(payload.to_string())
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}
