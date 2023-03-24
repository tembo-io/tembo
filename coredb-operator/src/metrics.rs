use crate::{apis::coredb_types::CoreDB, Error};
use kube::ResourceExt;
use prometheus::{histogram_opts, opts, HistogramVec, IntCounter, IntCounterVec, Registry};
use tokio::time::Instant;

#[derive(Clone)]
pub struct Metrics {
    pub reconciliations: IntCounter,
    pub failures: IntCounterVec,
    pub reconcile_duration: HistogramVec,
}

impl Default for Metrics {
    fn default() -> Self {
        let reconcile_duration = HistogramVec::new(
            histogram_opts!(
                "cdb_controller_reconcile_duration_seconds",
                "The duration of reconcile to complete in seconds"
            )
            .buckets(vec![0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.]),
            &[],
        )
        .unwrap();
        let failures = IntCounterVec::new(
            opts!(
                "cbd_controller_reconciliation_errors_total",
                "reconciliation errors",
            ),
            &["instance", "error"],
        )
        .unwrap();
        let reconciliations =
            IntCounter::new("cdb_controller_reconciliations_total", "reconciliations").unwrap();
        Metrics {
            reconciliations,
            failures,
            reconcile_duration,
        }
    }
}

impl Metrics {
    /// Register API metrics to start tracking them.
    pub fn register(self, registry: &Registry) -> Result<Self, prometheus::Error> {
        registry.register(Box::new(self.reconcile_duration.clone()))?;
        registry.register(Box::new(self.failures.clone()))?;
        registry.register(Box::new(self.reconciliations.clone()))?;
        Ok(self)
    }

    pub fn reconcile_failure(&self, cdb: &CoreDB, e: &Error) {
        self.failures
            .with_label_values(&[cdb.name_any().as_ref(), e.metric_label().as_ref()])
            .inc()
    }

    pub fn count_and_measure(&self) -> ReconcileMeasurer {
        self.reconciliations.inc();
        ReconcileMeasurer {
            start: Instant::now(),
            metric: self.reconcile_duration.clone(),
        }
    }
}

/// Smart function duration measurer
///
/// Relies on Drop to calculate duration and register the observation in the histogram
pub struct ReconcileMeasurer {
    start: Instant,
    metric: HistogramVec,
}

impl Drop for ReconcileMeasurer {
    fn drop(&mut self) {
        #[allow(clippy::cast_precision_loss)]
        let duration = self.start.elapsed().as_millis() as f64 / 1000.0;
        self.metric.with_label_values(&[]).observe(duration);
    }
}
