use pgx::bgworkers::*;
use pgx::prelude::*;
use pgx::spi::SpiTupleTable;

use std::time::Duration;

pgx::pg_module_magic!();

#[pg_extern]
fn hello_my_extension() -> &'static str {
    "Hello, my_extension"
}

type ExtensionRows = Vec<(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
)>;

#[pg_extern]
fn list_extensions() -> Result<
    TableIterator<
        'static,
        (
            name!(name, Option<String>),
            name!(default_version, Option<String>),
            name!(installed_version, Option<String>),
            name!(comment, Option<String>),
        ),
    >,
    spi::Error,
> {
    let results: Result<ExtensionRows, spi::Error> = Spi::connect(|mut client| {
        let mut results: ExtensionRows = Vec::new();
        let query = "select name::text, default_version, installed_version, comment from pg_catalog.pg_available_extensions".to_owned();
        let tup_table: SpiTupleTable = client.update(&query, None, None)?;
        for row in tup_table.into_iter() {
            let name = row["name"].value::<String>()?;
            let default_version = row["default_version"].value::<String>()?;
            let installed_version = row["installed_version"].value::<String>()?;
            let comment = row["comment"].value::<String>()?;
            results.push((name, default_version, installed_version, comment));
        }
        Ok(results)
    });
    Ok(TableIterator::new(results?.into_iter()))
}

#[allow(non_snake_case)]
#[pg_guard]
pub extern "C" fn _PG_init() {
    BackgroundWorkerBuilder::new("My Background Process for Postgres")
        .set_function("background_worker")
        .set_library("my_extension")
        .enable_spi_access()
        .set_start_time(BgWorkerStartTime::ConsistentState)
        .load();
}

#[pg_guard]
#[no_mangle]
pub extern "C" fn background_worker(_arg: pg_sys::Datum) {
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    BackgroundWorker::connect_worker_to_spi(Some("my_extension"), None);

    let setup_query = "create table if not exists my_test_table (id serial, name timestamp);";
    let _: Result<(), pgx::spi::Error> = BackgroundWorker::transaction(|| {
        Spi::connect(|mut client| {
            client.update(setup_query, None, None).unwrap();
            Ok(())
        })
    });

    // wake up every 10s or if we received a SIGTERM
    while BackgroundWorker::wait_latch(Some(Duration::from_secs(2))) {
        if BackgroundWorker::sighup_received() {
            // on SIGHUP, do something useful
        }

        let result: Result<(), pgx::spi::Error> = BackgroundWorker::transaction(|| {
            Spi::connect(|mut client| {
                let query = "insert into my_test_table (name) values (now());";
                client.update(query, None, None).unwrap();
                Ok(())
            })
        });
        result.unwrap_or_else(|e| panic!("got an error: {}", e))
    }

    log!("Closing BGWorker: {}", BackgroundWorker::get_name());
}


#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::prelude::*;

    #[pg_test]
    fn test_hello_my_extension() {
        assert_eq!("Hello, my_extension", crate::hello_my_extension());
    }
}

/// This module is required by `cargo pgx test` invocations.
/// It must be visible at the root of your extension crate.
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
