pub mod common {
    use actix_http::Request;
    use actix_service::Service;
    use actix_web::test;
    use actix_web::{dev::ServiceResponse, web, App, Error};

    #[cfg(test)]
    pub async fn get_test_app(
        auth: bool,
    ) -> impl Service<Request, Response = ServiceResponse, Error = Error> {
        let mut cfg = gateway::config::Config::new().await;
        cfg.org_auth_enabled = auth;
        let startup_config = gateway::server::webserver_startup_config(cfg).await;
        test::init_service(
            App::new()
                .app_data(web::Data::new(startup_config.cfg.clone()))
                .app_data(web::Data::new(startup_config.http_client.clone()))
                .app_data(web::Data::new(startup_config.pool.clone()))
                .app_data(web::Data::new(startup_config.auth_cache.clone()))
                .configure(gateway::server::webserver_routes),
        )
        .await
    }
}
