use actix_web::{dev::ServerHandle, middleware::Logger, web, App, HttpServer};
use kube::Client;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use opentelemetry::global;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    runtime,
    trace as sdktrace,
};
use parking_lot::Mutex;
use std::sync::Arc;
use tembo_pod_init::{config::Config, health::*, mutate::mutate, watcher::NamespaceWatcher, metrics};
use tracing::*;
use tracing_actix_web::{DefaultRootSpanBuilder, TracingLogger};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};
use uuid::Uuid;

#[instrument(fields(trace_id))]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::default();

    // Initialize logging and tracing
    init_telemetry(&config.opentelemetry_endpoint_url);

    // Set trace_id for logging
    let trace_id = Uuid::new_v4().to_string();
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

fn init_telemetry(otlp_endpoint_url: &Option<String>) {
    // Set up global propagator for distributed tracing context
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Create a standard JSON logger for stdout
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .json();

    // Get log level from environment or use default
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Set up Prometheus metrics
    let registry = prometheus::Registry::new();

    // Create the exporter without automatic resource metrics
    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .without_target_info() // This disables the target_info metric
        .build()
        .unwrap();

    let provider = SdkMeterProvider::builder()
        .with_reader(exporter)
        .build();
    global::set_meter_provider(provider);

    // Store registry in our global static
    *metrics::REGISTRY.lock().unwrap() = registry;

    // Initialize custom metrics
    metrics::init_metrics();

    // Initialize tracing only if endpoint is configured
    if let Some(endpoint) = otlp_endpoint_url {
        // Set up the tracer provider with OTLP exporter
        let tracer_provider = init_tracer_provider(endpoint);
        global::set_tracer_provider(tracer_provider);

        // Create a tracing layer with the configured tracer
        let telemetry_layer = tracing_opentelemetry::layer();

        // Register layers
        Registry::default()
            .with(env_filter)
            .with(fmt_layer)
            .with(telemetry_layer)
            .init();

        info!("Telemetry initialized with OpenTelemetry tracing to {}", endpoint);
    } else {
        // Just set up standard logging without OpenTelemetry
        Registry::default()
            .with(env_filter)
            .with(fmt_layer)
            .init();

        info!("Telemetry initialized with local logging only");
    }
}

fn init_tracer_provider(endpoint: &str) -> sdktrace::TracerProvider {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .unwrap();

    sdktrace::TracerProvider::builder()
        .with_resource(tembo_pod_init::metrics::BUILD_RESOURCE.clone())
        .with_batch_exporter(exporter, runtime::Tokio)
        .build()
}

struct StopHandle {
    inner: Mutex<Option<ServerHandle>>,
}

impl StopHandle {
    // Set the ServerHandle to stop
    pub(crate) fn register(&self, handle: ServerHandle) {
        *self.inner.lock() = Some(handle);
    }
}

impl Default for StopHandle {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
        }
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
