use actix_web::{web, App};

#[cfg(test)]
mod tests {
    use super::*;

    use actix_web::test;
    use dataplane_webserver::routes::health::{lively, ready};
    use dataplane_webserver::routes::root;

    #[actix_web::test]
    async fn test_probes() {
        env_logger::init();

        let app = test::init_service(
            App::new()
                .service(web::scope("/").service(root::ok))
                .service(web::scope("/health").service(ready).service(lively)),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::get().uri("/health/lively").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::get().uri("/health/ready").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}
