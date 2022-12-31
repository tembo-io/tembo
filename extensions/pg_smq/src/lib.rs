use pgx::prelude::*;
use pgx::warning;

pgx::pg_module_magic!();

const VT_DEFAULT: i64 = 60;

// read many messages at once, if they exist
#[pg_extern]
fn psmq_read_many(queue_name: &str, qty: i32) {
    // TODO - LIMIT {qty}
}

// change attributes on existing queue
#[pg_extern]
fn psmq_alter_queue(queue_name: &str) {
    // TODO
}

// changes VT on an existing message
#[pg_extern]
fn psmq_set_vt(queue_name: &str, msg_id: &str, vt: i64) {
    // TODO
}

#[pg_extern]
fn psmq_create(queue_name: &str) -> bool {
    Spi::run(&format!(
        "CREATE TABLE {name} (
            msg_id SERIAL,
            vt BIGINT,
            visible BOOL DEFAULT TRUE,
            message JSON
        );",
        name = queue_name
    ));
    true
}

// puts messages onto the queue
#[pg_extern]
fn psmq_enqueue(queue_name: &str, message: pgx::Json) -> Option<i64> {
    Spi::get_one(&format!(
        "INSERT INTO {queue_name} (vt, visible, message)
            VALUES ('{vt}', '{visible}', '{message}'::json)
            RETURNING msg_id;",
        queue_name = queue_name,
        vt = 1,
        visible = true,
        message = message.0,
    ))
}

// check message out of the queue
#[pg_extern]
fn psmq_read(queue_name: &str, vt: Option<i64>) -> pgx::Json {
    let _vt = vt.unwrap_or(VT_DEFAULT);

    let msg = Spi::get_one(&format!(
        "
        WITH cte AS
            (
                SELECT *
                FROM '{queue_name}'
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
        UPDATE '{queue_name}'
        SET visible = false, vt = {_vt}
        WHERE rank = (select rank from cte)
        RETURNING *;
        "
    ));
    msg.unwrap()
}

#[pg_extern]
fn psmq_delete(queue_name: &str, msg_id: String) -> bool {
    let del: Option<i32> = Spi::get_one(&format!(
        "
            DELETE
            FROM '{queue}'
            WHERE msg_id = '{msg_id}';
            ",
        queue = queue_name,
        msg_id = msg_id
    ));
    match del {
        Some(_) => true,
        None => {
            warning!("msg_id: {} not found in queue: {}", msg_id, queue_name);
            false
        }
    }
}

// reads and deletes at same time
#[pg_extern]
fn psmq_pop(queue_name: &str) -> pgx::Json {
    Spi::get_one(&format!(
        "
            WITH cte AS
                (
                    SELECT *
                    FROM '{queue_name}'
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
            UPDATE '{queue_name}'
            DELETE
            WHERE rank = (select rank from cte)
            RETURNING *;
            "
    ))
    .unwrap()
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::prelude::*;

    #[pg_test]
    fn test_create() {
        let qname = r#"test_queue"#;
        crate::psmq_create(&qname);
        let retval = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert_eq!(retval, 0);
        crate::psmq_enqueue(&qname, pgx::Json(serde_json::json!({"x":"y"})));
        let retval = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert_eq!(retval, 1);
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
