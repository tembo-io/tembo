use actix_web::{dev::ServerHandle, middleware::Logger, web, App, HttpServer};
use kube::Client;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use opentelemetry::global;
use opentelemetry_sdk::{runtime, trace::{self, Sampler}, propagation::TraceContextPropagator};
use opentelemetry_otlp::WithExportConfig;
use parking_lot::Mutex;
use std::sync::Arc;
use tembo_pod_init::{config::Config, health::*, mutate::mutate, watcher::NamespaceWatcher};
use tracing::*;
use tracing_actix_web::{TracingLogger, DefaultRootSpanBuilder};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};
use tracing_opentelemetry::OpenTelemetryLayer;
use uuid::Uuid;

const TRACER_NAME: &str = "tembo.io/tembo-pod-init";

#[instrument(fields(trace_id))]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Config::default();

    // Initialize logging and tracing
    init_telemetry(&config.opentelemetry_endpoint_url).await;

    // Set trace_id for logging
    let trace_id = Uuid::new_v4().to_string();
    Span::current().record("trace_id", &field::display(&trace_id));

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
        let trace_id_data = web::Data::new(trace_id.clone());
        move || {
            {
                App::new()
                    .app_data(config_data.clone())
                    .app_data(kube_data.clone())
                    .app_data(namespace_watcher_data.clone())
                    .app_data(stop_handle.clone())
                    .app_data(trace_id_data.clone())
                    .wrap(
                        TracingLogger::<DefaultRootSpanBuilder>::new()
                            .exclude("/health/liveness")
                            .exclude("/health/readiness")
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

async fn init_telemetry(otlp_endpoint_url: &Option<String>) {
    // Set up global propagator
    global::set_text_map_propagator(TraceContextPropagator::new());


    // Create a new OpenTelemetry pipeline
    let tracer = if let Some(endpoint) = otlp_endpoint_url {
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint)
            )
            .with_trace_config(
                trace::config()
                    .with_sampler(Sampler::AlwaysOn)
                    .with_id_generator(trace::RandomIdGenerator::default())
            )
            .install_batch(runtime::Tokio)
            .expect("Failed to create OpenTelemetry tracer");

        Some(tracer)
    } else {
        None
    };

    // Create a tracing layer with the configured tracer
    let telemetry_layer = match tracer {
        Some(tracer) => Some(OpenTelemetryLayer::new(tracer)),
        None => None,
    };

    // Get log level from environment or use default
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Create and register the subscriber
    let subscriber = Registry::default()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().json());

    // Conditionally add the OpenTelemetry layer
    if let Some(layer) = telemetry_layer {
        subscriber.with(layer).init();
    } else {
        subscriber.init();
    }

    info!("Telemetry initialized");
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
