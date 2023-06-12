use actix_web::{middleware, web, App, HttpServer};

use actix_cors::Cors;
use dataplane_webserver::{
    routes::health::{lively, ready},
    routes::root,
};

use utoipa::OpenApi;
use utoipa_swagger_ui::{SwaggerUi, Url};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));

    #[derive(OpenApi)]
    #[openapi(paths(), components(schemas()))]
    struct ApiDoc;

    HttpServer::new(move || {
        let mut doc = ApiDoc::openapi();
        doc.info.description = Some("Dataplane API".to_string());
        doc.info.license = None;

        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .service(web::scope("/").service(root::ok))
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
