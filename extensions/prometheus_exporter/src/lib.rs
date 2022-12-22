use pgx::bgworkers::*;
use pgx::log;
use pgx::prelude::*;

mod metrics;
mod webserver;

pgx::pg_module_magic!();

#[allow(non_snake_case)]
#[pg_guard]
pub extern "C" fn _PG_init() {
    BackgroundWorkerBuilder::new("Prometheus Exporter for Postgres")
        .set_function("background_worker")
        .set_library("prometheus_exporter")
        .enable_spi_access()
        .set_start_time(BgWorkerStartTime::ConsistentState)
        .load();
}

#[pg_guard]
#[no_mangle]
pub extern "C" fn background_worker(_arg: pg_sys::Datum) {
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    BackgroundWorker::connect_worker_to_spi(Some("prometheus_exporter"), None);

    webserver::run().unwrap();

    log!("Closing BGWorker: {}", BackgroundWorker::get_name());
}

// required by pgx for testing
#[cfg(test)]
pub mod pg_test {
    use once_cell::sync::Lazy;
    use pgx::bgworkers::*;

    static SHARED_PREPLOAD_LIB: Lazy<String> =
        Lazy::new(|| "shared_preload_libraries = 'prometheus_exporter.so'".to_string());

    pub fn setup(_options: Vec<&str>) {
        BackgroundWorkerBuilder::new("Prometheus Exporter for Postgres")
            .set_function("background_worker")
            .set_library("prometheus_exporter")
            .enable_spi_access()
            .set_start_time(BgWorkerStartTime::ConsistentState)
            .load();
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        vec![&*SHARED_PREPLOAD_LIB]
    }
}
