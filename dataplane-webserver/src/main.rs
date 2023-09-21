use actix_web::{middleware, web, App, HttpServer};

use actix_cors::Cors;

use dataplane_webserver::routes::secrets::AvailableSecret;
use dataplane_webserver::{
    config,
    routes::health::{lively, ready},
    routes::root,
};
use log::info;

use dataplane_webserver::routes::{metrics, secrets};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use utoipa_swagger_ui::{SwaggerUi, Url};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));

    let cfg = config::Config::default();
    info!("{:?}", cfg);

    // Initializing the HTTP client during server startup
    // allows for connection pooling and re-use of TCP
    // connections to the Prometheus server.
    let http_client = reqwest::Client::builder()
        .build()
        .expect("Failed to create HTTP client");

    #[derive(OpenApi)]
    #[openapi(
        paths(
              secrets::get_secret,
              secrets::get_secret_names,
              metrics::query_range,
        ),
        components(schemas(
            AvailableSecret
        )),
        modifiers(&SecurityAddon),
        security(("jwt_token" = [])),
    )]
    struct ApiDoc;

    struct SecurityAddon;

    impl Modify for SecurityAddon {
        fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
            let components = openapi.components.as_mut().unwrap();
            components.add_security_scheme(
                "jwt_token",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }

    HttpServer::new(move || {
        let mut doc = ApiDoc::openapi();
        doc.info.title = "Tembo Data API".to_string();
        doc.info.license = None;
        doc.info.description = Some("In the case of large or sensitive data, we avoid collecting it into Tembo Cloud. Instead, there is a Tembo Data API for each region, cloud, or private data plane.".to_string());
        doc.info.version = "v0.0.1".to_owned();

        let cors = Cors::permissive();
        App::new()
            .app_data(web::Data::new(cfg.clone()))
            .app_data(web::Data::new(http_client.clone()))
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .service(web::scope("/").service(root::ok))
            .service(web::scope("/{namespace}/metrics").service(metrics::query_range))
            .service(
                web::scope("/{namespace}/secrets")
                    .service(secrets::get_secret)
                    .service(secrets::get_secret_names),
            )
            .service(web::scope("/health").service(ready).service(lively))
            .service(SwaggerUi::new("/swagger-ui/{_:.*}").urls(vec![(
                Url::new("dataplane-api", "/api-docs/openapi.json"),
                doc,
            )]))
    })
    .workers(8)
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
