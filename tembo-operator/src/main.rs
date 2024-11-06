use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
pub use controller::{self, telemetry, State};

#[get("/metrics")]
async fn metrics(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let metrics = c.metrics();
    HttpResponse::Ok()
        .content_type("application/openmetrics-text; version=1.0.0; charset=utf-8")
        .body(metrics)
}

#[get("/health")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[get("/")]
async fn index(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}

async fn app_main() -> anyhow::Result<()> {
    telemetry::init().await;

    // Prepare shared state for the kubernetes controller and web server
    let state = State::default();
    let controller = controller::run(state.clone());

    let server_port = std::env::var("PORT")
        .unwrap_or_else(|_| String::from("8080"))
        .parse::<u16>()
        .unwrap_or(8080);

    // Start web server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(state.clone()))
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(index)
            .service(health)
            .service(metrics)
    })
    .bind(("0.0.0.0", server_port))?
    .shutdown_timeout(5);

    // Both runtimes implements graceful shutdown, so poll until both are done
    tokio::join!(controller, server.run()).1?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .build()?;
    rt.block_on(app_main())
}
