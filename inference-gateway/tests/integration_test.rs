mod util;

use actix_web::{http::header, test};
use rand::prelude::*;
use sqlx::Row;
use util::common;

use gateway::config::Config;
use gateway::db::connect;

#[ignore]
#[actix_web::test]
async fn test_probes() {
    let app = common::get_test_app().await;

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
    use env_logger;
    env_logger::init();
    let config = Config::new().await;
    let app = common::get_test_app().await;

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
    println!("Response: {:?}", resp);
    assert!(resp.status().is_success());
    // TOOD: parse resposne
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
