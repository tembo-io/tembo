use pgx::prelude::*;

pgx::pg_module_magic!();

#[pg_extern]
fn hello_my_extension() -> &'static str {
    "Hello, my_extension"
}




#[pg_extern]
fn list_extensions() -> Result<
TableIterator<
    'static,
    (
        name!(name, String),
        name!(default_version, String),
        name!(installed_version, String),
        name!(comment, String),
    ),
>,
spi::Error,
> {
    let query = "select name, default_version, installed_version, comment from pg_available_extensions;";
    let results: Result<Vec<(String, String, String, String)>, spi::Error> = Spi::connect(|client| {
        Ok(client.select(query, None, None)?.map(|row| (
            row["name"].value::<String>(),
            row["default_version"].value::<String>(),
            row["installed_version"].value::<String>(),
            row["comment"].value::<String>(),
        ))
        )
    });
    Ok(TableIterator::new(results?.into_iter()))
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
