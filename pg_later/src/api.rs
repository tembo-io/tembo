use pgrx::prelude::*;

#[pg_extern]
fn exec_query(
    query: &str,
) -> Result<TableIterator<'static, (name!(query, String), name!(results, pgrx::JsonB))>, spi::Error>
{
    let resultset = query_to_json(query)?;
    Ok(TableIterator::new(resultset.into_iter()))
}

fn query_to_json(query: &str) -> Result<Vec<(String, pgrx::JsonB)>, spi::Error> {
    let mut results: Vec<(String, pgrx::JsonB)> = Vec::new();
    let _: Result<(), spi::Error> = Spi::connect(|client| {
        let q = format!("select to_jsonb(t) as results from ({query}) t");
        let tup_table = client.select(&q, None, None)?;
        for row in tup_table {
            let r = row["results"]
                .value::<pgrx::JsonB>()
                .expect("failed parsing as json")
                .expect("no results from query");
            results.push(("Query".to_owned(), r));
        }
        Ok(())
    });
    Ok(results)
}
