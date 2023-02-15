use pgmq_crate::query::{
    create_archive, create_index, create_meta, insert_meta, SCHEMA, TABLE_PREFIX,
};

// for now, put pg_partman in the public schema
//
const PARTMAN_SCHEMA: &str = "public";

pub fn init_partitioned_queue(name: &str) -> Vec<String> {
    vec![
        create_meta(),
        create_partitioned_queue(name),
        create_partitioned_index(name),
        create_index(name),
        create_archive(name),
        create_partman(name),
        insert_meta(name),
    ]
}

pub fn create_partitioned_queue(queue: &str) -> String {
    format!(
        "
        CREATE TABLE IF NOT EXISTS {SCHEMA}.{TABLE_PREFIX}_{queue} (
            msg_id BIGSERIAL,
            read_ct INT DEFAULT 0,
            enqueued_at TIMESTAMP WITH TIME ZONE DEFAULT (now() at time zone 'utc'),
            vt TIMESTAMP WITH TIME ZONE,
            message JSON
        ) PARTITION BY RANGE (msg_id);;
        "
    )
}

pub fn create_partitioned_index(queue: &str) -> String {
    format!(
        "
        CREATE INDEX IF NOT EXISTS msg_id_idx_{queue} ON {SCHEMA}.{TABLE_PREFIX}_{queue} (msg_id);
        "
    )
}

pub fn create_partman(queue: &str) -> String {
    // TODO: 1000 is a placeholder. should be configurable and optimized default
    format!(
        "
        SELECT {PARTMAN_SCHEMA}.create_parent('{SCHEMA}.{TABLE_PREFIX}_{queue}', 'msg_id', 'native', '1000');
        "
    )
}
