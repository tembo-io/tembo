use actix_web::{web, App};

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use trunk_registry::routes::get_all_extensions;

    /// make sure the webserver boots up
    #[actix_web::test]
    async fn test_get_all_extensions() {
        env_logger::init();

        let cfg = trunk_registry::config::Config::default();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(cfg.clone()))
                .service(get_all_extensions),
        )
        .await;
        // good request should succeed
        let req = test::TestRequest::get().uri("/extensions/all").to_request();
        let resp = test::call_service(&app, req).await;
        println!("status: {:?}", resp.response());
        assert!(resp.status().is_success());

        // bad request should be client error
        let req = test::TestRequest::get()
            .uri("/extensions/get-all")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_client_error());
    }
}
