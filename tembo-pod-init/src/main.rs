use actix_web::{dev::ServerHandle, middleware::Logger, web, App, HttpServer};
use kube::Client;
use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};
use opentelemetry::global;
use parking_lot::Mutex;
use std::sync::Arc;
use tembo_pod_init::{
    config::Config, health::*, metrics, mutate::mutate, telemetry, watcher::NamespaceWatcher,
};
use tracing::*;
use tracing_actix_web::{DefaultRootSpanBuilder, TracingLogger};
use uuid::Uuid;

async fn setup_kubernetes_client() -> Result<Client, kube::Error> {
    Client::try_default().await
}

#[instrument(fields(trace_id))]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::default();

    // Initialize logging and tracing
    let telemetry = telemetry::Telemetry::default();
    telemetry.init(&config.opentelemetry_endpoint_url);

    // Set trace_id for logging
    let trace_id = Uuid::new_v4().to_string();
    Span::current().record("trace_id", field::display(&trace_id));

    let stop_handle = web::Data::new(StopHandle::default());

    // Setup Kubernetes Client
    let kube_client = match setup_kubernetes_client().await {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Kubernetes client: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create Kubernetes client: {}", e),
            ));
        }
    };

    // Start watching namespaces in a seperate tokio task thread
    let watcher = NamespaceWatcher::new(Arc::new(kube_client.clone()), config.clone());
    let namespaces = watcher.get_namespaces();
    tokio::spawn(watch_namespaces(watcher));

    // Load the TLS certificate and key
    let tls_config = match setup_tls_config(&config) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to set up TLS configuration: {}", e);
            return Err(e);
        }
    };
    let server_bind_address = format!("{}:{}", config.server_host, config.server_port);

    let server = HttpServer::new({
        let config_data = web::Data::new(config.clone());
        let kube_data = web::Data::new(Arc::new(kube_client.clone()));
        let namespace_watcher_data = web::Data::new(namespaces.clone());
        let stop_handle = stop_handle.clone();
        let trace_id_data = web::Data::new(trace_id.clone());
        move || {
            {
                App::new()
                    .app_data(config_data.clone())
                    .app_data(kube_data.clone())
                    .app_data(namespace_watcher_data.clone())
                    .app_data(stop_handle.clone())
                    .app_data(trace_id_data.clone())
                    .wrap(TracingLogger::<DefaultRootSpanBuilder>::new())
                    .wrap(
                        Logger::new("%a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T")
                            .exclude("/health/liveness")
                            .exclude("/health/readiness")
                            .exclude("/metrics"),
                    )
                    .service(liveness)
                    .service(readiness)
                    .service(mutate)
                    .service(metrics::metrics)
            }
        }
    })
    .bind_openssl(server_bind_address, tls_config)?
    .shutdown_timeout(5)
    .run();

    stop_handle.register(server.handle());

    info!(
        "Starting HTTPS server at https://{}:{}/",
        config.server_host, config.server_port
    );
    debug!("Config: {:?}", config);
    server.await?;

    // Make sure we close all the spans
    global::shutdown_tracer_provider();

    Ok(())
}

#[derive(Default)]
struct StopHandle {
    inner: Mutex<Option<ServerHandle>>,
}

impl StopHandle {
    // Set the ServerHandle to stop
    pub(crate) fn register(&self, handle: ServerHandle) {
        *self.inner.lock() = Some(handle);
    }
}

#[instrument(skip(watcher))]
async fn watch_namespaces(watcher: NamespaceWatcher) {
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);
    const MAX_CONSECUTIVE_ERRORS: usize = 10;

    let mut consecutive_errors = 0;

    loop {
        match watcher.watch().await {
            Ok(_) => {
                info!("Namespace watcher finished, restarting.");
                consecutive_errors = 0; // Reset error counter on success
            }
            Err(e) => {
                consecutive_errors += 1;
                error!(
                    "Namespace watcher failed (attempt {}/{}), restarting: {}",
                    consecutive_errors, MAX_CONSECUTIVE_ERRORS, e
                );

                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    error!("Too many consecutive errors in namespace watcher, giving up");
                    break;
                }

                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
}

fn setup_tls_config(config: &Config) -> Result<SslAcceptorBuilder, std::io::Error> {
    let mut tls_config = SslAcceptor::mozilla_intermediate(SslMethod::tls())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    tls_config
        .set_private_key_file(&config.tls_key, SslFiletype::PEM)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    tls_config
        .set_certificate_chain_file(&config.tls_cert)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(tls_config)
}
