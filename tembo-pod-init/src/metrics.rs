use once_cell::sync::Lazy;
use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use prometheus::{opts, Encoder, GaugeVec, HistogramVec, IntCounterVec, Registry, TextEncoder};
use std::sync::Mutex;
use tracing::error;

// A global registry for Prometheus metrics
pub static REGISTRY: Lazy<Mutex<Registry>> = Lazy::new(|| Mutex::new(Registry::new()));

// A global resource for Prometheus metrics
pub static BUILD_RESOURCE: Lazy<Resource> = Lazy::new(|| {
    Resource::new(vec![
        KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAME,
            "tembo.io/tembo-pod-init",
        ),
        KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_VERSION,
            env!("CARGO_PKG_VERSION"),
        ),
        // Rust build information
        KeyValue::new("rust_version", rustc_version_runtime::version().to_string()),
        KeyValue::new(
            "build_profile",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        ),
        // Git information (if available via build-time env vars)
        KeyValue::new(
            "git_commit",
            option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),
        ),
        KeyValue::new("git_branch", option_env!("GIT_BRANCH").unwrap_or("unknown")),
        // Build timestamp
        KeyValue::new("build_timestamp", env!("CARGO_PKG_VERSION")),
        // Runtime information
        KeyValue::new("host_os", std::env::consts::OS),
        KeyValue::new("host_arch", std::env::consts::ARCH),
    ])
});

pub static REQUEST_COUNTER: Lazy<Option<IntCounterVec>> = Lazy::new(|| {
    match IntCounterVec::new(
        opts!(
            "tembo_pod_init_requests_total",
            "Total number of admission requests processed"
        ),
        &["namespace", "operation", "resource", "result"],
    ) {
        Ok(counter) => Some(counter),
        Err(e) => {
            error!("Failed to create REQUEST_COUNTER metric: {}", e);
            None
        }
    }
});

pub static REQUEST_DURATION: Lazy<Option<HistogramVec>> = Lazy::new(|| {
    match HistogramVec::new(
        prometheus::HistogramOpts::new(
            "tembo_pod_init_request_duration_seconds",
            "Duration of admission request processing in seconds",
        )
        .buckets(vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]),
        &["namespace", "operation", "resource"],
    ) {
        Ok(histogram) => Some(histogram),
        Err(e) => {
            error!("Failed to create REQUEST_DURATION metric: {}", e);
            None
        }
    }
});

pub static MUTATION_COUNTER: Lazy<Option<IntCounterVec>> = Lazy::new(|| {
    match IntCounterVec::new(
        opts!(
            "tembo_pod_init_mutations_total",
            "Total number of mutations performed"
        ),
        &["namespace", "resource", "mutation_type"],
    ) {
        Ok(counter) => Some(counter),
        Err(e) => {
            error!("Failed to create MUTATION_COUNTER metric: {}", e);
            None
        }
    }
});

pub static ERROR_COUNTER: Lazy<Option<IntCounterVec>> = Lazy::new(|| {
    match IntCounterVec::new(
        opts!(
            "tembo_pod_init_errors_total",
            "Total number of errors encountered"
        ),
        &["namespace", "error_type"],
    ) {
        Ok(counter) => Some(counter),
        Err(e) => {
            error!("Failed to create ERROR_COUNTER metric: {}", e);
            None
        }
    }
});

pub static VOLUME_MUTATIONS: Lazy<Option<IntCounterVec>> = Lazy::new(|| {
    match IntCounterVec::new(
        opts!(
            "tembo_pod_init_volume_mutations_total",
            "Total number of volume mutations"
        ),
        &["namespace", "volume_type"],
    ) {
        Ok(counter) => Some(counter),
        Err(e) => {
            error!("Failed to create VOLUME_MUTATIONS metric: {}", e);
            None
        }
    }
});

// Global build info metric
pub static BUILD_INFO: Lazy<GaugeVec> = Lazy::new(|| {
    let build_info = GaugeVec::new(
        opts!(
            "tembo_pod_init_info",
            "Build information for tembo-pod-init"
        ),
        &[
            "service_name",
            "version",
            "rust_version",
            "git_commit",
            "git_branch",
            "build_profile",
            "host_os",
            "host_arch",
        ],
    )
    .unwrap();

    // Set the value to 1 with all the labels
    build_info
        .with_label_values(&[
            "tembo.io/tembo-pod-init",
            env!("CARGO_PKG_VERSION"),
            &rustc_version_runtime::version().to_string(),
            option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),
            option_env!("GIT_BRANCH").unwrap_or("unknown"),
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            std::env::consts::OS,
            std::env::consts::ARCH,
        ])
        .set(1.0);

    build_info
});

// Helper functions to safely increment metrics
pub fn increment_request_counter(namespace: &str, operation: &str, resource: &str, result: &str) {
    if let Some(counter) = &*REQUEST_COUNTER {
        counter
            .with_label_values(&[namespace, operation, resource, result])
            .inc();
    }
}

pub fn observe_request_duration(namespace: &str, operation: &str, resource: &str, duration: f64) {
    if let Some(histogram) = &*REQUEST_DURATION {
        histogram
            .with_label_values(&[namespace, operation, resource])
            .observe(duration);
    }
}

pub fn increment_mutation_counter(namespace: &str, resource: &str, mutation_type: &str) {
    if let Some(counter) = &*MUTATION_COUNTER {
        counter
            .with_label_values(&[namespace, resource, mutation_type])
            .inc();
    }
}

pub fn increment_error_counter(namespace: &str, error_type: &str) {
    if let Some(counter) = &*ERROR_COUNTER {
        counter.with_label_values(&[namespace, error_type]).inc();
    }
}

pub fn increment_volume_mutations(namespace: &str, volume_type: &str) {
    if let Some(counter) = &*VOLUME_MUTATIONS {
        counter.with_label_values(&[namespace, volume_type]).inc();
    }
}

// Initialize metrics
pub fn init_metrics() {
    let registry_lock = match REGISTRY.lock() {
        Ok(lock) => lock,
        Err(e) => {
            error!("Failed to lock metrics registry: {}", e);
            return;
        }
    };

    // Register all metrics with the registry
    if let Some(counter) = &*REQUEST_COUNTER {
        if let Err(e) = registry_lock.register(Box::new(counter.clone())) {
            error!("Failed to register REQUEST_COUNTER: {}", e);
        }
    }

    if let Some(histogram) = &*REQUEST_DURATION {
        if let Err(e) = registry_lock.register(Box::new(histogram.clone())) {
            error!("Failed to register request duration histogram: {}", e);
        }
    }

    if let Some(counter) = &*MUTATION_COUNTER {
        if let Err(e) = registry_lock.register(Box::new(counter.clone())) {
            error!("Failed to register mutation counter: {}", e);
        }
    }

    if let Some(counter) = &*ERROR_COUNTER {
        if let Err(e) = registry_lock.register(Box::new(counter.clone())) {
            error!("Failed to register error counter: {}", e);
        }
    }

    if let Some(counter) = &*VOLUME_MUTATIONS {
        if let Err(e) = registry_lock.register(Box::new(counter.clone())) {
            error!("Failed to register volume mutations counter: {}", e);
        }
    }

    // Register BUILD_INFO and set values
    if let Err(e) = registry_lock.register(Box::new(BUILD_INFO.clone())) {
        error!("Failed to register BUILD_INFO: {}", e);
    } else {
        // Set the build info values
        BUILD_INFO
            .with_label_values(&[
                "tembo.io/tembo-pod-init",
                env!("CARGO_PKG_VERSION"),
                &rustc_version_runtime::version().to_string(),
                option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),
                option_env!("GIT_BRANCH").unwrap_or("unknown"),
                if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                },
                std::env::consts::OS,
                std::env::consts::ARCH,
            ])
            .set(1.0);
    }

    // Add webhook availability metric
    match prometheus::register_gauge_vec_with_registry!(
        opts!(
            "tembo_pod_init_webhook_up",
            "Whether the admission webhook is up (1) or down (0)"
        ),
        &["endpoint"],
        &*registry_lock
    ) {
        Ok(gauge) => {
            gauge.with_label_values(&["default"]).set(1.0);
        }
        Err(e) => {
            error!("Failed to register webhook_up gauge: {}", e);
        }
    }
}

// Add a route to expose Prometheus metrics
#[actix_web::get("/metrics")]
pub async fn metrics() -> impl actix_web::Responder {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.lock().unwrap().gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    actix_web::HttpResponse::Ok()
        .content_type("text/plain")
        .body(buffer)
}
