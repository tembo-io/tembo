use std::sync::{Arc, Mutex};

use pgx::bgworkers::*;
use pgx::prelude::*;

pub const UPTIME_QUERY: &str =
    "SELECT FLOOR(EXTRACT(EPOCH FROM now() - pg_postmaster_start_time))::bigint
FROM pg_postmaster_start_time();";

// executes queries specifically for computation of metrics
// for now, only supports metrics which are updated/set with i64 values
pub fn handle_query(query: &str) -> Option<i64> {
    let uptime = Arc::new(Mutex::new(i64::default()));
    let clone = Arc::clone(&uptime);
    // interacting with the SPI bust be done in a background worker
    BackgroundWorker::transaction(move || {
        let mut obj_clone = clone.lock().unwrap();
        *obj_clone = query_exec(&query).unwrap();
    });
    let x = Some(*uptime.lock().unwrap());
    x
}

fn query_exec(query: &str) -> Option<i64> {
    Spi::get_one(&query)
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use crate::metrics::query;
    use pgx::prelude::*;

    #[pg_test]
    fn test_query_exec() {
        assert!(query::query_exec(query::UPTIME_QUERY).is_some());
    }
}
