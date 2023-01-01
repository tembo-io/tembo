use pgx::prelude::*;
use pgx::warning;

pgx::pg_module_magic!();

const VT_DEFAULT: i64 = 30;
const DELAY_DEFAULT: i64 = 0;

// read many messages at once, if they exist
#[pg_extern]
fn pgmq_read_many(_queue_name: &str, _qty: i32) {
    // TODO - LIMIT {qty}
}

// change attributes on existing queue
#[pg_extern]
fn pgmq_alter_queue(_queue_name: &str) {
    // TODO
}

// changes VT on an existing message
#[pg_extern]
fn pgmq_set_vt(_queue_name: &str, _msg_id: &str, _vt: i64) {
    // TODO
}

#[pg_extern]
fn pgmq_create(queue_name: &str) -> bool {
    Spi::run(&format!(
        "
        CREATE TABLE IF NOT EXISTS pgmq_config (
            queue_name VARCHAR NOT NULL,
            vt_default INT DEFAULT {VT_DEFAULT},
            delay_default INT DEFAULT {DELAY_DEFAULT},
            created TIMESTAMP DEFAULT current_timestamp
        );

        CREATE TABLE IF NOT EXISTS {name} (
            msg_id BIGSERIAL,
            vt TIMESTAMP,
            message JSON
        );

        INSERT INTO pgmq_config (queue_name)
        VALUES ('{name}');
        ",
        name = queue_name
    ));
    // TODO: create index on vt
    true
}

// puts messages onto the queue
#[pg_extern]
fn pgmq_enqueue(queue_name: &str, message: pgx::Json) -> Option<i64> {
    // TODO: impl delay on top of vt
    Spi::get_one(&format!(
        "INSERT INTO {queue_name} (vt, message)
            VALUES (now() at time zone 'utc', '{message}'::json)
            RETURNING msg_id;",
        queue_name = queue_name,
        message = message.0,
    ))
}

// check message out of the queue using default timeout
#[pg_extern]
fn pgmq_read(queue_name: &str) -> Option<pgx::Json> {
    let (msg_id, vt, message) = Spi::get_three::<i64, pgx::Timestamp, pgx::Json>(&format!(
        "
    WITH cte AS
        (
            SELECT msg_id, vt, message
            FROM {queue_name}
            WHERE vt <= now() at time zone 'utc'
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
    UPDATE {queue_name}
    SET vt = (now() at time zone 'utc' + interval '{VT_DEFAULT} seconds')
    WHERE msg_id = (select msg_id from cte)
    RETURNING *;
    "
    ));
    match msg_id {
        Some(msg_id) => Some(pgx::Json(serde_json::json!({
            "msg_id": msg_id,
            "vt": vt,
            "message": message
        }))),
        None => None,
    }
}

#[pg_extern]
fn pgmq_delete(queue_name: &str, msg_id: &str) -> bool {
    let del: Option<i32> = Spi::get_one(&format!(
        "
            DELETE
            FROM {queue}
            WHERE msg_id = '{msg_id}';
            ",
        queue = queue_name,
        msg_id = msg_id
    ));
    match del.unwrap() {
        d if d >= 1 => true,
        _ => {
            warning!("msg_id: {} not found in queue: {}", msg_id, queue_name);
            false
        }
    }
}

// reads and deletes at same time
#[pg_extern]
fn pgmq_pop(queue_name: &str) -> Option<pgx::Json> {
    let (msg_id, vt, message) = Spi::get_three::<i64, pgx::Timestamp, pgx::Json>(&format!(
        "
            WITH cte AS
                (
                    SELECT msg_id, vt, message
                    FROM {queue_name}
                    WHERE vt <= now() at time zone 'utc'
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
            DELETE from {queue_name}
            WHERE msg_id = (select msg_id from cte)
            RETURNING *;
        "
    ));
    match msg_id {
        Some(msg_id) => Some(pgx::Json(serde_json::json!({
            "msg_id": msg_id,
            "vt": vt,
            "message": message
        }))),
        None => None,
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::prelude::*;

    #[pg_test]
    fn test_create() {
        let qname = r#"test_queue"#;
        crate::pgmq_create(&qname);
        let retval = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert_eq!(retval, 0);
        crate::pgmq_enqueue(&qname, pgx::Json(serde_json::json!({"x":"y"})));
        let retval = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert_eq!(retval, 1);
    }

    // assert an invisible message is not readable
    #[pg_test]
    fn test_default_vt() {
        let qname = r#"test_queue"#;
        crate::pgmq_create(&qname);
        let init_count = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        // should not be any messages initially
        assert_eq!(init_count, 0);

        // put a message on the queue
        crate::pgmq_enqueue(&qname, pgx::Json(serde_json::json!({"x":"y"})));
        // read the message off queue
        let msg = crate::pgmq_read(&qname);
        assert!(msg.is_some());
        // should be no messages left
        let nomsgs = crate::pgmq_read(&qname);
        assert!(nomsgs.is_none());
        // but still one record on the table
        let init_count = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert_eq!(init_count, 1);
    }
}

#[cfg(test)]
pub mod pg_test {
    // pg_test module with both the setup and postgresql_conf_options functions are required
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
