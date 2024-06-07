use opentelemetry::metrics::{Counter, Meter};

#[derive(Clone)]
pub struct CustomMetrics {
    pub conductor: Counter<u64>,
    pub conductor_requeues: Counter<u64>,
    pub conductor_errors: Counter<u64>,
    pub conductor_completed: Counter<u64>,
}

impl CustomMetrics {
    pub fn new(meter: &Meter) -> Self {
        let conductor = meter
            .u64_counter("conductor_total")
            .with_description("Total number of dequeues in conductor")
            .init();
        let conductor_requeues = meter
            .u64_counter("conductor_requeues")
            .with_description("Number of requeues in conductor")
            .init();
        let conductor_errors = meter
            .u64_counter("conductor_errors")
            .with_description("Number of errors in conductor")
            .init();
        let conductor_completed = meter
            .u64_counter("conductor_completed")
            .with_description("Number of messages successfully processed in conductor")
            .init();
        Self {
            conductor,
            conductor_requeues,
            conductor_errors,
            conductor_completed,
        }
    }
}
