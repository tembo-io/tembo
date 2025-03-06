use actix_web::{dev::ServerHandle, web, App, HttpServer};
use kube::Client;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use opentelemetry::global;
use parking_lot::Mutex;
use std::sync::Arc;
use tembo_pod_init::{config::Config, health::*, mutate::mutate, watcher::NamespaceWatcher};
use tembo_telemetry::{TelemetryConfig, TelemetryInit};
use tracing::*;

const TRACER_NAME: &str = "tembo.io/tembo-pod-init";

#[instrument(fields(trace_id))]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Config::default();

    // Initialize logging
    let otlp_endpoint_url = &config.opentelemetry_endpoint_url;
    let telemetry_config = TelemetryConfig {
        app_name: "tembo-pod-init".to_string(),
        env: std::env::var("ENV").unwrap_or_else(|_| "production".to_string()),
        endpoint_url: otlp_endpoint_url.clone(),
        tracer_id: Some(TRACER_NAME.to_string()),
    };

    let _ = TelemetryInit::init(&telemetry_config).await;

    // Set trace_id for logging
    let trace_id = telemetry_config.get_trace_id();
    Span::current().record("trace_id", field::display(&trace_id));

    let stop_handle = web::Data::new(StopHandle::default());

    // Setup Kubernetes Client
    let kube_client = match Client::try_default().await {
        Ok(client) => client,
        Err(e) => {
            panic!("Failed to create Kubernetes client: {}", e);
        }
    };

    // Start watching namespaces in a seperate tokio task thread
    let watcher = NamespaceWatcher::new(Arc::new(kube_client.clone()), config.clone());
    let namespaces = watcher.get_namespaces();
    tokio::spawn(watch_namespaces(watcher));

    // Load the TLS certificate and key
    let mut tls_config = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    tls_config
        .set_private_key_file(config.tls_key.clone(), SslFiletype::PEM)
        .unwrap();
    tls_config
        .set_certificate_chain_file(config.tls_cert.clone())
        .unwrap();
    let server_bind_address = format!("{}:{}", config.server_host, config.server_port);

    let server = HttpServer::new({
        let config_data = web::Data::new(config.clone());
        let kube_data = web::Data::new(Arc::new(kube_client.clone()));
        let namespace_watcher_data = web::Data::new(namespaces.clone());
        let stop_handle = stop_handle.clone();
        let tc = web::Data::new(telemetry_config.clone());
        move || {
            {
                App::new()
                    .app_data(config_data.clone())
                    .app_data(kube_data.clone())
                    .app_data(namespace_watcher_data.clone())
                    .app_data(stop_handle.clone())
                    .app_data(tc.clone())
                    .wrap(
                        tembo_telemetry::get_tracing_logger()
                            .exclude("/health/liveness")
                            .exclude("/health/readiness")
                            .build(),
                    )
                    .service(liveness)
                    .service(readiness)
                    .service(mutate)
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
    loop {
        match watcher.watch().await {
            Ok(_) => {
                info!("Namespace watcher finished, restarting.");
            }
            Err(e) => {
                error!("Namespace watcher failed, restarting: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}
