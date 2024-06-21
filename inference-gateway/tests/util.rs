pub mod common {
    use actix_http::Request;
    use actix_service::Service;
    use actix_web::test;
    use actix_web::{dev::ServiceResponse, web, App, Error};
    use sqlx::{Pool, Postgres};

    #[cfg(test)]
    pub async fn get_test_app() -> impl Service<Request, Response = ServiceResponse, Error = Error>
    {
        let config = gateway::config::Config::new().await;
        let dbclient: Pool<Postgres> = gateway::db::connect(&config.pg_conn_str, 4)
            .await
            .expect("Failed to connect to database");
        let reqwest_client: reqwest::Client = reqwest::Client::new();

        sqlx::migrate!("./migrations");

        test::init_service(
            App::new()
                .app_data(web::Data::new(config.clone()))
                .app_data(web::Data::new(reqwest_client.clone()))
                .app_data(web::Data::new(dbclient.clone()))
                .configure(gateway::server::webserver_routes),
        )
        .await
    }
}
