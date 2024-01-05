use crate::{apis::coredb_types::CoreDB, psql::PsqlOutput, Context};
use kube::{runtime::controller::Action, ResourceExt};
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{debug, warn};

const HEARTBEAT_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION run_heartbeat()
RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE
    schema_exists BOOLEAN;
    table_exists BOOLEAN;
BEGIN
    -- Check if schema exists
    SELECT EXISTS(
        SELECT schema_name
        FROM information_schema.schemata
        WHERE schema_name = 'tembo'
    ) INTO schema_exists;

    -- Create schema if it doesn't exist
    IF NOT schema_exists THEN
        EXECUTE 'CREATE SCHEMA tembo;';
    END IF;

    -- Check if table exists within tembo schema
    SELECT EXISTS(
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'tembo' AND table_name = 'heartbeat_table'
    ) INTO table_exists;

    -- Create table and index if they don't exist
    IF NOT table_exists THEN
        EXECUTE 'CREATE TABLE tembo.heartbeat_table (
            latest_heartbeat TIMESTAMP NOT NULL
        );';
        EXECUTE 'CREATE INDEX idx_heartbeat ON tembo.heartbeat_table (latest_heartbeat);';
    END IF;

    -- Insert current UTC timestamp into heartbeat_table
    EXECUTE 'INSERT INTO tembo.heartbeat_table (latest_heartbeat)
        VALUES (CURRENT_TIMESTAMP AT TIME ZONE ''UTC'');';

    -- Delete entries older than 7 days
    EXECUTE 'DELETE FROM tembo.heartbeat_table
        WHERE latest_heartbeat < (CURRENT_TIMESTAMP AT TIME ZONE ''UTC'' - INTERVAL ''7 days'');';

END;
$$;
"#;

// reconcile_heartbeat is a function to run the setup_heartbeat function on the database instance
// and then run the run_heartbeat function to insert a timestamp into the heartbeat_table.
pub async fn reconcile_heartbeat(coredb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    // Match to make sure the HEARTBEAT_FUNCTION is installed on the database instance, requeue if
    // it fails for some reason.
    match setup_heartbeat(coredb, ctx.clone()).await {
        Ok(_) => debug!(
            "Successfully created setup_heartbeat function on instance {}",
            coredb.name_any()
        ),
        Err(e) => {
            warn!(
                "Did not create setup_heartbeat function, will requeue: {:?}",
                e
            );
            return Err(Action::requeue(Duration::from_secs(30)));
        }
    }
    // Run the setup_pgbouncer function
    coredb
        .psql(
            "SELECT run_heartbeat();".to_string(),
            "postgres".to_string(),
            ctx.clone(),
        )
        .await?;

    Ok(())
}

// setup_heartbeat is a function to create a schema and table to write to everytime there is a
// reconciliation loop.
async fn setup_heartbeat(coredb: &CoreDB, ctx: Arc<Context>) -> Result<PsqlOutput, Action> {
    // Install or update the HEARTBEAT_FUNCTION function on the database instance
    let query = coredb
        .psql(
            HEARTBEAT_FUNCTION.to_string(),
            "postgres".to_string(),
            ctx.clone(),
        )
        .await?;
    Ok(query)
}
