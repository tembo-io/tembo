use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    metrics::SdkMeterProvider, propagation::TraceContextPropagator, runtime, trace as sdktrace,
};
use std::error::Error;
use tracing::*;
use tracing_subscriber::{
    fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry,
};

use crate::metrics;

/// Telemetry manager for handling tracing, metrics, and logging
pub struct Telemetry {
    tracer_name: &'static str,
}

impl Telemetry {
    /// Create a new telemetry manager
    pub fn new(tracer_name: &'static str) -> Self {
        Self { tracer_name }
    }

    /// Initialize telemetry with optional OpenTelemetry endpoint
    pub fn init(&self, otlp_endpoint_url: &Option<String>) {
        // Set up global propagator for distributed tracing context
        global::set_text_map_propagator(TraceContextPropagator::new());

        // Create a standard JSON logger for stdout
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout)
            .json()
            .with_span_events(FmtSpan::NONE);

        // Get log level from environment or use default
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        // Set up Prometheus metrics
        let registry = prometheus::Registry::new();

        // Create the exporter without automatic resource metrics
        let exporter = match opentelemetry_prometheus::exporter()
            .with_registry(registry.clone())
            .without_target_info()
            .build()
        {
            Ok(exporter) => exporter,
            Err(e) => {
                error!("Failed to build Prometheus exporter: {}", e);
                return;
            }
        };

        let provider = SdkMeterProvider::builder().with_reader(exporter).build();
        global::set_meter_provider(provider);

        // Store registry in our global static
        match metrics::REGISTRY.lock() {
            Ok(mut lock) => *lock = registry,
            Err(e) => error!("Failed to lock metrics registry: {}", e),
        }

        // Initialize custom metrics
        metrics::init_metrics();

        // Initialize tracing only if endpoint is configured
        if let Some(endpoint) = otlp_endpoint_url {
            // Set up the tracer provider with OTLP exporter
            let tracer_provider = match self.init_tracer_provider(endpoint) {
                Ok(provider) => provider,
                Err(err) => {
                    error!("Failed to initialize tracer provider: {}", err);
                    // Create a fallback provider that doesn't export traces
                    sdktrace::TracerProvider::builder()
                        .with_resource(crate::metrics::BUILD_RESOURCE.clone())
                        .build()
                }
            };

            global::set_tracer_provider(tracer_provider);

            // Create a tracing layer with the configured tracer
            let telemetry_layer = tracing_opentelemetry::layer();

            // Register layers
            Registry::default()
                .with(env_filter)
                .with(fmt_layer)
                .with(telemetry_layer)
                .init();

            info!(
                "Telemetry initialized with OpenTelemetry tracing to {}",
                endpoint
            );
        } else {
            // Just set up standard logging without OpenTelemetry
            Registry::default().with(env_filter).with(fmt_layer).init();

            info!("Telemetry initialized with local logging only");
        }
    }

    /// Initialize tracer provider with OTLP exporter
    fn init_tracer_provider(
        &self,
        endpoint: &str,
    ) -> Result<sdktrace::TracerProvider, Box<dyn Error + Send + Sync>> {
        let exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()?;

        let provider = sdktrace::TracerProvider::builder()
            .with_resource(opentelemetry_sdk::Resource::new(vec![KeyValue::new(
                opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                self.tracer_name.to_string(),
            )]))
            .with_batch_exporter(exporter, runtime::Tokio)
            .build();

        Ok(provider)
    }

    /// Shutdown telemetry, ensuring all spans are flushed
    pub fn shutdown(&self) {
        info!("Shutting down telemetry");
        global::shutdown_tracer_provider();
    }
}

/// Default implementation for convenience
impl Default for Telemetry {
    fn default() -> Self {
        Self::new("tembo.io/tembo-pod-init")
    }
}
