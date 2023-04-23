use pgx;
use pgx::prelude::*;

use pgmq_crate::{
    errors::PgmqError,
    query::{check_input, create_archive, create_index, create_meta, insert_meta, TABLE_PREFIX},
};

// for now, put pg_partman in the public PGMQ_SCHEMA
const PARTMAN_SCHEMA: &str = "public";
const PGMQ_SCHEMA: &str = "public";

pub fn init_partitioned_queue(
    name: &str,
    partition_size: &str,
    retention_size: &str,
) -> Result<Vec<String>, PgmqError> {
    check_input(name)?;
    Ok(vec![
        create_meta(),
        create_partitioned_queue(name)?,
        create_partitioned_index(name)?,
        create_index(name)?,
        create_archive(name)?,
        create_partman(name, partition_size)?,
        insert_meta(name)?,
        set_retention_config(name, retention_size)?,
    ])
}

// set retention policy for a queue
// retention policy is only used for partition maintenance
// messages .deleted() are immediately removed from the queue
// messages .archived() will be retained forever on the `<queue_name>_archive` table
// https://github.com/pgpartman/pg_partman/blob/ca212077f66af19c0ca317c206091cd31d3108b8/doc/pg_partman.md#retention
// integer value will set that any partitions with an id value less than the current maximum id value minus the retention value will be dropped
pub fn set_retention_config(queue: &str, retention: &str) -> Result<String, PgmqError> {
    check_input(queue)?;
    Ok(format!(
        "
        ALTER {PGMQ_SCHEMA}.part_config
        SET 
            retention = '{retention}',
            retention_keep_table = false,
            retention_keep_index = true,
            automatic_maintenance = 'on'
        WHERE parent_table = {PGMQ_SCHEMA}.{TABLE_PREFIX}_{queue}
        "
    ))
}

pub fn create_partitioned_queue(queue: &str) -> Result<String, PgmqError> {
    check_input(queue)?;
    Ok(format!(
        "
        CREATE TABLE IF NOT EXISTS {PGMQ_SCHEMA}.{TABLE_PREFIX}_{queue} (
            msg_id BIGSERIAL,
            read_ct INT DEFAULT 0,
            enqueued_at TIMESTAMP WITH TIME ZONE DEFAULT (now() at time zone 'utc'),
            vt TIMESTAMP WITH TIME ZONE,
            message JSONB
        ) PARTITION BY RANGE (msg_id);;
        "
    ))
}

pub fn create_partitioned_index(queue: &str) -> Result<String, PgmqError> {
    check_input(queue)?;
    Ok(format!(
        "
        CREATE INDEX IF NOT EXISTS msg_id_idx_{queue} ON {PGMQ_SCHEMA}.{TABLE_PREFIX}_{queue} (msg_id);
        "
    ))
}

pub fn create_partman(queue: &str, partition_size: &str) -> Result<String, PgmqError> {
    check_input(queue)?;
    let partition_col = match partition_size.parse::<i64>() {
        Ok(_) => "msg_id",
        Err(_) => "enqueued_at",
    };
    Ok(format!(
        "
        SELECT {PARTMAN_SCHEMA}.create_parent('{PGMQ_SCHEMA}.{TABLE_PREFIX}_{queue}', '{partition_col}', 'native', '{partition_size}');
        "
    ))
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;
    #[pg_test]
    fn test_create_partman() {
        let query = create_partman("test", "1 day").unwrap();
        assert!(query.contains("enqueued_at"));

        let query = create_partman("test", "100").unwrap();
        assert!(query.contains("msg_id"));
    }
}
