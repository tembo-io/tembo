use pgx::bgworkers::*;
use pgx::log;
use pgx::prelude::*;

mod webserver;

pgx::pg_module_magic!();

#[allow(non_snake_case)]
#[pg_guard]
pub extern "C" fn _PG_init() {
    BackgroundWorkerBuilder::new("Prometheus Exporter for Postgres")
        .set_function("serve_metrics")
        .set_library("prometheus_exporter")
        .enable_spi_access()
        .set_start_time(BgWorkerStartTime::ConsistentState)
        .load();
}

#[pg_guard]
#[no_mangle]
pub extern "C" fn serve_metrics(_arg: pg_sys::Datum) {
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    BackgroundWorker::connect_worker_to_spi(Some("prometheus_exporter"), None);

    webserver::serve().unwrap();

    log!("Closing BGWorker: {}", BackgroundWorker::get_name());
}

#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
