use crate::PgmqExtError;
use pgmq_crate::query::{check_input};

pub const TABLE_PREFIX: &str = r#"pg_stream"#;
const SCHEMA: &str = "public";


pub fn create_partitioned_stream(queue: &str) -> Result<String, PgmqExtError> {
    check_input(queue)?;
    Ok(format!(
        "
        CREATE TABLE IF NOT EXISTS {SCHEMA}.{TABLE_PREFIX}_{queue} (
            msg_id BIGSERIAL,
            sent_at TIMESTAMP WITH TIME ZONE DEFAULT (now() at time zone 'utc'),
            message JSONB
        ) PARTITION BY RANGE (msg_id);;
        "
    ))
}

pub fn create_partitioned_stream_index(queue: &str) -> Result<String, PgmqExtError> {
    check_input(queue)?;
    Ok(format!(
        "
        CREATE INDEX IF NOT EXISTS msg_id_idx_{queue} ON {SCHEMA}.{TABLE_PREFIX}_{queue} (msg_id);
        "
    ))
}

pub fn create_partman_stream(queue: &str, partition_size: i64) -> Result<String, PgmqError> {
    check_input(queue)?;
    Ok(format!(
        "
        SELECT {PARTMAN_SCHEMA}.create_parent('{SCHEMA}.{TABLE_PREFIX}_{queue}', 'msg_id', 'native', '{partition_size}');
        "
    ))
}


pub fn subscribe(queue: &str, group: &str) -> Result<String, PgmqExtError> {
    check_input(queue)?;
    Ok(format!(
        "
        CREATE TABLE IF NOT EXISTS {SCHEMA}.{TABLE_PREFIX}_groups_{queue} (
            group_name VARCHAR(255) NOT NULL,
            sequence bigint,





            
        );
        "
    ))
}