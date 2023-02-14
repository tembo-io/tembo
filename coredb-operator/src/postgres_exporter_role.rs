use crate::{Context, CoreDB, Error};
use std::sync::Arc;
use tracing::debug;

pub async fn create_postgres_exporter_role(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<(), Error> {
    let client = &ctx.client;
    debug!(
        "Creating postgres_exporter role for database {} in namespace {}",
        cdb.metadata.name.clone().unwrap(),
        cdb.metadata.namespace.clone().unwrap()
    );
    // https://github.com/prometheus-community/postgres_exporter#running-as-non-superuser
    let _ = cdb
        .psql(
            "
            CREATE OR REPLACE FUNCTION __tmp_create_user() returns void as $$
            BEGIN
              IF NOT EXISTS (
                      SELECT
                      FROM   pg_catalog.pg_user
                      WHERE  usename = 'postgres_exporter') THEN
                CREATE USER postgres_exporter;
              END IF;
            END;
            $$ language plpgsql;

            SELECT __tmp_create_user();
            DROP FUNCTION __tmp_create_user();

            ALTER USER postgres_exporter SET SEARCH_PATH TO postgres_exporter,pg_catalog;
            GRANT CONNECT ON DATABASE postgres TO postgres_exporter;
            GRANT pg_monitor to postgres_exporter;
            GRANT pg_read_all_stats to postgres_exporter;
            "
            .to_string(),
            "postgres".to_owned(),
            client.clone(),
        )
        .await
        .unwrap();
    Ok(())
}
