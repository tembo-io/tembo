use pgx::prelude::*;
use pgx::spi::SpiTupleTable;
use pgx::warning;

pgx::pg_module_magic!();

use pgmq::query::{create, delete, enqueue_str, read};

const VT_DEFAULT: i64 = 30;
const DELAY_DEFAULT: i64 = 0;

// read many messages at once, if they exist
#[pg_extern]
fn pgmq_read_many(_queue_name: &str, _qty: i32) {
    !todo!()
}

// change attributes on existing queue
#[pg_extern]
fn pgmq_alter_queue(_queue_name: &str) {
    !todo!()
}

// changes VT on an existing message
#[pg_extern]
fn pgmq_set_vt(_queue_name: &str, _msg_id: &str, _vt: i64) {
    !todo!()
}

#[pg_extern]
fn pgmq_create(queue_name: &str) -> Result<(), pgx::spi::Error> {
    Spi::run(&create(&queue_name))
}

// puts messages onto the queue
#[pg_extern]
fn pgmq_enqueue(queue_name: &str, message: pgx::Json) -> Result<Option<i64>, spi::Error> {
    let m = serde_json::to_string(&message.0).unwrap();
    Spi::get_one(&enqueue_str(queue_name, &m))
}

// check message out of the queue using default timeout
#[pg_extern]
fn pgmq_read(queue_name: &str, vt: i32) -> Result<Option<pgx::Json>, spi::Error> {
    let (msg_id, vt, message) =
        Spi::get_three::<i64, pgx::TimestampWithTimeZone, pgx::Json>(&read(queue_name, &vt))?;

    match msg_id {
        Some(msg_id) => Ok(Some(pgx::Json(serde_json::json!({
            "msg_id": msg_id,
            "vt": vt.unwrap(),
            "message": message.unwrap()
        })))),
        None => Ok(None),
    }
}

#[pg_extern(volatile)]
fn pgmq_delete(queue_name: &str, msg_id: i64) -> Result<Option<bool>, spi::Error> {
    let mut num_deleted = 0;

    Spi::connect(|client| {
        let tup_table = client.select(&delete(queue_name, &msg_id), None, None);
        match tup_table {
            Ok(tup_table) => num_deleted = tup_table.len(),
            Err(e) => {
                error!("error deleting message: {}", e);
            }
        }
    });
    match num_deleted {
        1 => Ok(Some(true)),
        0 => {
            warning!("no message found with msg_id: {}", msg_id);
            Ok(Some(false))
        }
        _ => {
            error!("multiple messages found with msg_id: {}", msg_id);
        }
    }
}

// reads and deletes at same time
// #[pg_extern]
// fn pgmq_pop(queue_name: &str) -> Option<pgx::Json> {
//     let (msg_id, vt, message) = Spi::get_three::<i64, pgx::Timestamp, pgx::Json>(&format!(
//         "
//             WITH cte AS
//                 (
//                     SELECT msg_id, vt, message
//                     FROM {queue_name}
//                     WHERE vt <= now() at time zone 'utc'
//                     LIMIT 1
//                     FOR UPDATE SKIP LOCKED
//                 )
//             DELETE from {queue_name}
//             WHERE msg_id = (select msg_id from cte)
//             RETURNING *;
//         "
//     ));
//     match msg_id {
//         Some(msg_id) => Some(pgx::Json(serde_json::json!({
//             "msg_id": msg_id,
//             "vt": vt,
//             "message": message
//         }))),
//         None => None,
//     }
// }

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
        assert!(retval.is_none());
        assert_eq!(retval.unwrap(), 0);
        crate::pgmq_enqueue(&qname, pgx::Json(serde_json::json!({"x":"y"})));
        let retval = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert!(retval.is_none());
        assert_eq!(retval.unwrap(), 0);
    }

    // assert an invisible message is not readable
    #[pg_test]
    fn test_default_vt() {
        let qname = r#"test_queue"#;
        crate::pgmq_create(&qname);
        let init_count = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        // should not be any messages initially
        assert!(init_count.is_none());

        // put a message on the queue
        crate::pgmq_enqueue(&qname, pgx::Json(serde_json::json!({"x":"y"})));
        // read the message off queue
        let msg = crate::pgmq_read(&qname, 10_i32).unwrap();
        // assert!(msg.);
        // should be no messages left
        let nomsgs = crate::pgmq_read(&qname, 10_i32);
        // assert!(nomsgs.is_none());
        // but still one record on the table
        let init_count = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        // assert_eq!(init_count, 1);
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
