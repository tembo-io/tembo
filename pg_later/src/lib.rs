use pgrx::{prelude::*, JsonB};
use serde_json::json;

pgrx::pg_module_magic!();
use serde::{Deserialize, Serialize};

use serde_json::Value;


#[pg_extern]
fn execute_and_return_json(query: &str) -> String {
    let spi_result: Option<pgrx::JsonB> = Spi::get_one(&format!("SELECT to_jsonb(t) FROM ({query}) t")).expect("Failed to execute query");

    // If the result is not None, convert to JSON string, otherwise return an empty string.
    match spi_result {
        Some(jsonb_result) => serde_json::to_string(&jsonb_result).expect("Failed to serialize JSON"),
        None => String::from(""),
    }
}

#[pg_extern]
fn exec_query(query: &str) -> Result<
TableIterator<
    'static,
        (   
            name!(query, String),
            name!(results, pgrx::JsonB),
        ),
>,
spi::Error,
> {
    let resultset = query_to_json(query)?;
    // Ok(TableIterator::from_vec(vec!["results"], resultset))
    Ok(TableIterator::new(resultset.into_iter()))
}


fn query_to_json(query: &str) -> Result<
Vec<
   (
    String,
    pgrx::JsonB
),
>,
spi::Error,
> {
    let mut results: Vec<(String, pgrx::JsonB)> = Vec::new();
    let _: Result<(), spi::Error> = Spi::connect(|client| {
        let q = format!("select to_jsonb(t) as results from ({query}) t");
        let mut tup_table = client.select(&q, None, None)?;
        while let Some(row) = tup_table.next() {
            let r = row["results"].value::<pgrx::JsonB>().expect("no res").unwrap();
            results.push(("Query".to_owned(), r));
        }
        Ok(())
    });
    Ok(results)
}


#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    // #[pg_test]
    // fn test_hello_pg_later() {
    //     assert_eq!("Hello, pg_later", crate::hello_pg_later());
    // }

}

/// This module is required by `cargo pgrx test` invocations.
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
