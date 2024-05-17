mod util;

use actix_web::test;
use util::common;

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
