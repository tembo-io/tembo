#![allow(unused_imports)] // some used only for telemetry feature
use opentelemetry::trace::{TraceId, TracerProvider};
use opentelemetry_sdk::{runtime, trace as sdktrace, trace::Config, Resource};
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

///  Fetch an opentelemetry::trace::TraceId as hex through the full tracing stack
pub fn get_trace_id() -> TraceId {
    use opentelemetry::trace::TraceContextExt as _; // opentelemetry::Context -> opentelemetry::trace::Span
    use tracing_opentelemetry::OpenTelemetrySpanExt as _; // tracing::Span to opentelemetry::Context
    tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
}

#[cfg(feature = "telemetry")]
fn resource() -> Resource {
    use opentelemetry::KeyValue;
    Resource::new([
        KeyValue::new("service.name", env!("CARGO_PKG_NAME")),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ])
}

#[cfg(feature = "telemetry")]
fn init_tracer() -> sdktrace::Tracer {
    use opentelemetry_otlp::WithExportConfig;
    let endpoint = std::env::var("OPENTELEMETRY_ENDPOINT_URL").expect("Needs an otel collector");
    let exporter = opentelemetry_otlp::new_exporter().tonic().with_endpoint(endpoint);

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(Config::default().with_resource(resource()))
        .install_batch(runtime::Tokio)
        .expect("valid tracer");

    opentelemetry::global::set_tracer_provider(provider.clone());
    provider.tracer("tracing-otel-subscriber")
}

/// Initialize tracing
pub async fn init() {
    // Setup tracing layers
    #[cfg(feature = "telemetry")]
    let otel = tracing_opentelemetry::OpenTelemetryLayer::new(init_tracer());

    let logger = tracing_subscriber::fmt::layer().compact();
    let env_filter = EnvFilter::try_from_default_env()
        .or(EnvFilter::try_new("info"))
        .unwrap();

    // Decide on layers
    let reg = Registry::default();
    #[cfg(feature = "telemetry")]
    reg.with(env_filter).with(logger).with(otel).init();
    #[cfg(not(feature = "telemetry"))]
    reg.with(env_filter).with(logger).init();
}

#[cfg(test)]
mod test {
    // This test only works when telemetry is initialized fully
    // and requires OPENTELEMETRY_ENDPOINT_URL pointing to a valid server
    #[cfg(feature = "telemetry")]
    #[tokio::test]
    #[ignore = "requires a trace exporter"]
    async fn get_trace_id_returns_valid_traces() {
        use super::*;
        super::init().await;
        #[tracing::instrument(name = "test_span")] // need to be in an instrumented fn
        fn test_trace_id() -> TraceId {
            get_trace_id()
        }
        assert_ne!(test_trace_id(), TraceId::INVALID, "valid trace");
    }
}
