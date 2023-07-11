use pgrx::prelude::*;
use pgrx::spi;
use std::env;

use std::time::Duration;

use pgrx::bgworkers::*;

use crate::api::{delete_from_queue, get_job, query_to_json};

#[pg_guard]
pub extern "C" fn _PG_init() {
    BackgroundWorkerBuilder::new("PG Later Background Worker")
        .set_function("background_worker_main")
        .set_library("pg_later")
        // .set_argument(42i32.into_datum())
        .enable_spi_access()
        .load();
}

#[pg_guard]
#[no_mangle]
pub extern "C" fn background_worker_main(_arg: pg_sys::Datum) {

    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    // specify database
    // TODO: default to public schema
    let db = from_env_default("PG_LATER_DATABASE", "pg_later");
    BackgroundWorker::connect_worker_to_spi(Some(&db), None);

    log!("Starting BG Workers {}", BackgroundWorker::get_name(),);

    // poll at 10s or on a SIGTERM
    while BackgroundWorker::wait_latch(Some(Duration::from_secs(10))) {
        if BackgroundWorker::sighup_received() {
            // on SIGHUP, you might want to reload some external configuration or something
        }
        // within a transaction, execute an SQL statement, and log its results
        let _result: Result<(), pgrx::spi::Error> = BackgroundWorker::transaction(|| {
            let job = get_job(1);
            log!("job: {:?}", job);
            match job {
                Some((job_id, query)) => {
                    let _exec_job = exec_job(job_id, &query);
                    delete_from_queue(job_id)?;
                }
                None => {
                    log!("No job");
                }
            }
            Ok(())
        });
    }

    log!(
        "Goodbye from inside the {} BGWorker! ",
        BackgroundWorker::get_name()
    );
}

// executes a query and writes results to a results queue
fn exec_job(job_id: i64, query: &str) -> Result<(), spi::Error> {
    let result_message = match query_to_json(query) {
        Ok(json) => {
            serde_json::json!({
                "status": "success",
                "job_id": job_id,
                "query": query,
                "result": json,
            })
        }
        Err(e) => {
            log!("Error: {:?}", e);
            serde_json::json!({
                "status": "failure",
                "job_id": job_id,
                "query": query,
                "result": format!("error: {e}"),
            })
        }
    };
    let enqueue = format!("select pgmq_send('pg_later_results', '{result_message}')");
    let _: i64 = Spi::get_one(&enqueue)?.expect("query did not return message id");
    Ok(())
}

fn from_env_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}
