use crate::errors::PgmqError;
use crate::util::{connect, fetch_one_message};
use crate::Message;
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::Utc;
use sqlx::{Pool, Postgres};

/// Main controller for interacting with a managed by the PGMQ Postgres extension.
#[derive(Clone, Debug)]
pub struct PGMQueueExt {
    pub url: String,
    pub connection: Pool<Postgres>,
}

pub struct PGMQueueMeta {
    pub queue_name: String,
    pub created_at: chrono::DateTime<Utc>,
}
impl PGMQueueExt {
    /// Initialize a connection to PGMQ/Postgres
    pub async fn new(url: String, max_connections: u32) -> Result<PGMQueueExt, PgmqError> {
        Ok(PGMQueueExt {
            url: url.clone(),
            connection: connect(&url, max_connections).await?,
        })
    }

    /// Create a new partitioned queue.
    pub async fn create(&self, queue_name: &str) -> Result<(), PgmqError> {
        sqlx::query!("SELECT * from pgmq_create($1::text);", queue_name)
            .execute(&self.connection)
            .await?;
        Ok(())
    }

    /// Drop an existing queue table.
    pub async fn drop_queue(&self, queue_name: &str) -> Result<(), PgmqError> {
        sqlx::query!("SELECT * from pgmq_drop_queue($1::text);", queue_name)
            .fetch_optional(&self.connection)
            .await?;
        Ok(())
    }

    /// List all queues in the Postgres instance.
    pub async fn list_queues(&self) -> Result<Option<Vec<PGMQueueMeta>>, PgmqError> {
        let queues = sqlx::query_as!(PGMQueueMeta, "SELECT * from pgmq_meta;")
            .fetch_all(&self.connection)
            .await?;
        if queues.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(queues))
        }
    }

    // Set the visibility time on an existing message.
    pub async fn set_vt(
        &self,
        queue_name: String,
        msg_id: i64,
        vt: i32,
    ) -> Result<Message, PgmqError> {
        let updated = sqlx::query!(
            "SELECT * from pgmq_set_vt($1::text, $2, $3);",
            queue_name,
            msg_id,
            vt
        )
        .fetch_one(&self.connection)
        .await?;
        Ok(Message {
            msg_id: updated.msg_id.expect("msg_id missing"),
            vt: updated.vt.expect("vt missing"),
            read_ct: updated.read_ct.expect("read_ct missing"),
            enqueued_at: updated.enqueued_at.expect("enqueued_at missing"),
            message: serde_json::from_value(updated.message.expect("no message"))
                .expect("message missing"),
        })
    }

    pub async fn send<T: Serialize>(
        &self,
        queue_name: &str,
        message: &T,
    ) -> Result<i64, PgmqError> {
        let msg = serde_json::json!(&message);
        let sent = sqlx::query!(
            "SELECT pgmq_send as msg_id from pgmq_send($1::text, $2::jsonb);",
            queue_name,
            msg
        )
        .fetch_one(&self.connection)
        .await?;
        Ok(sent.msg_id.expect("no message id"))
    }

    pub async fn read<T: for<'de> Deserialize<'de>>(
        &self,
        queue_name: &str,
        vt: i32,
    ) -> Result<Option<Message<T>>, PgmqError> {
        let row = sqlx::query!(
            "SELECT * from pgmq_read($1::text, $2, $3)",
            queue_name,
            vt,
            1
        )
        .fetch_optional(&self.connection)
        .await?;
        match row {
            Some(row) => {
                // happy path - successfully read a message
                let raw_msg = row.message.expect("no message");
                let parsed_msg = serde_json::from_value::<T>(raw_msg)?;
                Ok(Some(Message {
                    msg_id: row.msg_id.expect("msg_id missing from queue table"),
                    vt: row.vt.expect("vt missing from queue table"),
                    read_ct: row.read_ct.expect("read_ct missing from queue table"),
                    enqueued_at: row
                        .enqueued_at
                        .expect("enqueued_at missing from queue table"),
                    message: parsed_msg,
                }))
            }
            None => {
                // no message found
                Ok(None)
            }
        }
    }
    pub async fn pop(queue_name: &str) -> String {
        String::from("")
    }
    pub async fn delete(queue_name: &str, msg_id: u32) -> bool {
        true
    }

    //
    pub async fn archive(queue_name: &str, msg_id: u32) -> bool {
        true
    }
}
