use pgrx::prelude::*;
use pgrx::spi::SpiTupleTable;

#[pg_extern]
fn pg_later_init() -> Result<bool, spi::Error> {
    let setup_queries = vec![
        "select pgmq_create_non_partitioned('pg_later_jobs')",
        "select pgmq_create_non_partitioned('pg_later_results')",
    ];
    for q in setup_queries {
        let ran: Result<_, spi::Error> = Spi::connect(|mut c| {
            let _ = c.update(q, None, None)?;
            Ok(())
        });
        ran?
    }
    Ok(true)
}

/// send a query to be executed by the next available worker
#[pg_extern]
pub fn pg_later_exec(query: &str) -> Result<i64, spi::Error> {
    let msg = serde_json::json!({
        "query": query,
    });
    let enqueue = format!("select pgmq_send('pg_later_jobs', '{msg}')");
    let msg_id: i64 = Spi::get_one(&enqueue)?.expect("failed to send message to queue");
    Ok(msg_id)
}

// get the resultset of a previously submitted query
#[pg_extern]
fn pg_later_results(job_id: i64) -> Result<Option<pgrx::JsonB>, spi::Error> {
    let query = format!(
        "select * from pgmq_pg_later_results
        where message->>'job_id' = '{job_id}'
        "
    );
    let results: Result<Option<pgrx::JsonB>, spi::Error> = Spi::connect(|mut client| {
        let mut tup_table: SpiTupleTable = client.update(&query, None, None)?;
        if let Some(row) = tup_table.next() {
            let message = row["message"].value::<pgrx::JsonB>()?.expect("no message");
            return Ok(Some(message));
        }
        Ok(None)
    });
    let query_resultset = match results {
        Ok(Some(r)) => r,
        Ok(None) => {
            return Ok(None);
        }
        _ => {
            return Err(spi::Error::CursorNotFound(
                "failed to execute query".to_owned(),
            ));
        }
    };
    Ok(Some(query_resultset))
}

// gets a job query from the queue
pub fn get_job(timeout: i64) -> Option<(i64, String)> {
    let job = poll_queue(timeout).expect("failed");
    match job {
        Some(job) => {
            let msg_id = job[0].0;
            let m = serde_json::to_value(&job[0].1).expect("failed parsing jsonb");
            let q = m["query"].as_str().expect("no query").to_owned();
            Some((msg_id, q))
        }
        None => None,
    }
}

fn poll_queue(timeout: i64) -> Result<Option<Vec<(i64, pgrx::JsonB)>>, spi::Error> {
    let mut results: Vec<(i64, pgrx::JsonB)> = Vec::new();

    let query =
        format!("select msg_id, message from public.pgmq_read('pg_later_jobs' ,{timeout}, 1)");
    let _: Result<(), spi::Error> = Spi::connect(|mut client| {
        let tup_table: SpiTupleTable = client.update(&query, None, None)?;
        for row in tup_table {
            let msg_id = row["msg_id"].value::<i64>()?.expect("no msg_id");
            let message = row["message"].value::<pgrx::JsonB>()?.expect("no message");
            results.push((msg_id, message));
        }
        Ok(())
    });
    if results.is_empty() {
        Ok(None)
    } else {
        Ok(Some(results))
    }
}

// #[pg_extern]
// pub fn exec_to_table(
//     query: &str,
// ) -> Result<TableIterator<'static, (name!(query, String), name!(results, pgrx::JsonB))>, spi::Error>
// {
//     let resultset = query_to_json(query)?;
//     Ok(TableIterator::new(resultset.into_iter()))
// }

use std::panic::{self};

pub fn query_to_json(query: &str) -> Result<Vec<pgrx::JsonB>, spi::Error> {
    let mut results: Vec<pgrx::JsonB> = Vec::new();
    log!("executing query: {query}");
    let queried = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let queried: Result<(), spi::Error> = Spi::connect(|mut client| {
            let q = format!("select to_jsonb(t) as results from ({query}) t");
            let tup_table = client.update(&q, None, None)?;
            for row in tup_table {
                let r = row["results"]
                    .value::<pgrx::JsonB>()
                    .expect("failed parsing as json")
                    .expect("no results from query");
                results.push(r);
            }
            Ok(())
        });
        queried
    }));
    match queried {
        Ok(_) => Ok(results),
        Err(e) => {
            log!("Error: {:?}", e);
            // TODO: a more appropriate error enum
            Err(spi::Error::CursorNotFound(
                "failed to execute query".to_owned(),
            ))
        }
    }
}

pub fn delete_from_queue(msg_id: i64) -> Result<(), spi::Error> {
    let del = format!("select pgmq_delete('pg_later_jobs', {msg_id})");
    let _: bool = Spi::get_one(&del)?.expect("failed to send message to queue");
    Ok(())
}

extension_sql!(
    "
    CREATE EXTENSION IF NOT EXISTS pgmq CASCADE;
    CREATE SCHEMA IF NOT EXISTS pglater;
    CREATE TABLE IF NOT EXISTS pglater.later_meta (
        id serial PRIMARY KEY,
        name text NOT NULL,
        description text,
        created_at timestamp NOT NULL DEFAULT now(),
        updated_at timestamp NOT NULL DEFAULT now()
    );",
    name = "pg_later_setup",
    bootstrap,
);
