use actix_web::{middleware, web, App, HttpServer};

use actix_cors::Cors;

use dataplane_webserver::secrets::types::{AvailableSecret, PasswordString};
use dataplane_webserver::{
    config,
    routes::health::{lively, ready},
    routes::root,
};
use log::info;

use dataplane_webserver::routes::{backups, metrics, secrets};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_redoc::{Redoc, Servable};

use utoipa_swagger_ui::{SwaggerUi, Url};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));
    rustls::crypto::CryptoProvider::install_default(rustls::crypto::aws_lc_rs::default_provider())
        .expect("Failed to install rustls CryptoProvider");

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
              secrets::get_secret_v1,
              secrets::get_secret_names_v1,
              secrets::update_postgres_password,
              metrics::query_range,
              metrics::query,
        ),
        components(schemas(
            AvailableSecret,
            PasswordString
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
        doc.info.description = Some(
            r#"In the case of large or sensitive data, we avoid collecting it into Tembo Cloud. Instead, there is a Tembo Data API for each region, cloud, or private data plane.
            </br>
            </br>
            To find the Tembo Cloud API, please find it [here](https://api.tembo.io/swagger-ui/).
            "#.to_string()
        );
        doc.info.version = "v0.0.1".to_owned();
        let mut redoc_docs = doc.clone();
        redoc_docs.info.description = Some(
            r#"In the case of large or sensitive data, we avoid collecting it into Tembo Cloud. Instead, there is a Tembo Data API for each region, cloud, or private data plane.
            </br>
            </br>
            To find the Tembo Cloud API, please find it [here](https://api.tembo.io/redoc).
            "#.to_string()
        );

        let cors = Cors::permissive();
        App::new()
            .app_data(web::Data::new(cfg.clone()))
            .app_data(web::Data::new(http_client.clone()))
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .service(web::scope("/").service(root::ok))
            .service(
                web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(secrets::get_secret_names_v1)
                    .service(secrets::get_secret_v1)
                    .service(secrets::update_postgres_password)
                    .service(backups::trigger_instance_backup)
                    .service(backups::get_backup_status)
            )
            .service(
                web::scope("/{namespace}/metrics")
                    .service(metrics::query_range)
                    .service(metrics::query),
            )
            .service(web::scope("/health").service(ready).service(lively))
            .service(SwaggerUi::new("/swagger-ui/{_:.*}").urls(vec![(
                Url::new("dataplane-api", "/api-docs/openapi.json"),
                doc.clone(),
            )]))
            .service(Redoc::with_url("/redoc", redoc_docs.clone()))
    })
    .workers(8)
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
