pub mod common {
    use actix_http::Request;
    use actix_service::Service;
    use actix_web::test;
    use actix_web::{dev::ServiceResponse, web, App, Error};

    #[cfg(test)]
    pub async fn get_test_app(
        validation: bool,
    ) -> impl Service<Request, Response = ServiceResponse, Error = Error> {
        let mut startup_config = gateway::server::webserver_startup_config().await;
        startup_config.cfg.org_validation_enabled = validation;
        test::init_service(
            App::new()
                .app_data(web::Data::new(startup_config.cfg.clone()))
                .app_data(web::Data::new(startup_config.http_client.clone()))
                .app_data(web::Data::new(startup_config.pool.clone()))
                .app_data(web::Data::new(startup_config.validation_cache.clone()))
                .configure(gateway::server::webserver_routes),
        )
        .await
    }
}
