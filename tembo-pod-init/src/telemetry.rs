use crate::config::Config;
use opentelemetry::trace::TraceId;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

#[cfg(feature = "telemetry")]
use opentelemetry::{
    global,
    sdk::{propagation::TraceContextPropagator, trace, trace::Sampler, Resource},
    KeyValue,
};

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
async fn init_tracer(config: &Config) -> opentelemetry::sdk::trace::Tracer {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let otlp_endpoint = &config.opentelemetry_endpoint_url;

    if otlp_endpoint.is_empty() {
        panic!("OPENTELEMETRY_ENDPOINT_URL is not set");
    }

    let channel = tonic::transport::Channel::from_shared(otlp_endpoint.to_string())
        .unwrap()
        .connect()
        .await
        .unwrap();

    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_channel(channel),
        )
        .with_trace_config(
            trace::config()
                .with_resource(Resource::new(vec![KeyValue::new(
                    "service.name",
                    "tembo-pod-init",
                )]))
                .with_sampler(Sampler::AlwaysOn),
        )
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap()
}

/// Initialize tracing
pub async fn init(config: &Config) {
    // Setup tracing layers
    #[cfg(feature = "telemetry")]
    let telemetry = tracing_opentelemetry::layer().with_tracer(init_tracer(config).await);
    let logger = tracing_subscriber::fmt::layer().compact();
    let env_filter = EnvFilter::new(&config.log_level);

    // Decide on layers
    #[cfg(feature = "telemetry")]
    let collector = Registry::default()
        .with(telemetry)
        .with(logger)
        .with(env_filter);
    #[cfg(not(feature = "telemetry"))]
    let collector = Registry::default().with(logger).with(env_filter);

    // Initialize tracing
    tracing::subscriber::set_global_default(collector).unwrap();
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
