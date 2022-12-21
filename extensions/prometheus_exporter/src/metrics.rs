// https://github.com/prometheus/client_rust/blob/master/examples/actix-web.rs
use signal_hook::flag;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;

pub mod query;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct UptimeLabels {
    pub label: String,
}

pub struct Metrics {
    pub uptime: Family<(), Gauge>,
}

impl Metrics {
    pub fn pg_uptime(&self, uptime: i64) {
        self.uptime.get_or_create(&()).set(uptime);
    }
}

pub async fn update_metrics(metrics_clone: Arc<Mutex<Metrics>>) {
    let term = Arc::new(AtomicBool::new(false));
    flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term)).unwrap();

    let metrics = metrics_clone.lock().unwrap();
    while !term.load(Ordering::Relaxed) {
        let uptime: i64 = query::handle_query(query::UPTIME_QUERY).unwrap();
        {
            metrics.pg_uptime(uptime);
            thread::sleep(time::Duration::from_millis(2500));
        }
    }
}
