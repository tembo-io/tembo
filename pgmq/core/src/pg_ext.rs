use sqlx::{Pool, Postgres};
use crate::util::connect;
use crate::errors::PgmqError;
use sqlx::types::chrono::Utc;

/// Main controller for interacting with a managed by the PGMQ Postgres extension.
#[derive(Clone, Debug)]
pub struct PGMQueueExt {
    pub url: String,
    pub connection: Pool<Postgres>,
}

pub struct PGMQueueMeta {
    pub queue_name: String,
    pub created_at: chrono::DateTime<Utc>
}
impl PGMQueueExt {
    /// Initialize a connection to PGMQ/Postgres
    pub async fn new(url: String, max_connections: u32) -> Result<PGMQueueExt, PgmqError>{
        Ok(
            PGMQueueExt {
                url: url.clone(),
                connection: connect(&url, max_connections).await?,
            }
        )
    }

    /// Create a new partitioned queue.
    pub async fn create(&self, queue_name: &str) -> Result<(), PgmqError> {
        sqlx::query!("SELECT * from pgmq_create($1::text);", queue_name).execute(&self.connection).await?;
        Ok(())
    }

    /// Drop an existing queue table.
    pub async fn drop_queue(&self, queue_name: &str) -> Result<(), PgmqError> {
        sqlx::query!("SELECT * from pgmq_drop_queue($1::text);", queue_name).fetch_optional(&self.connection).await?;
        Ok(())
    }

    /// List all queues in the Postgres instance.
    pub async fn list_queues(&self) -> Result<Option<Vec<PGMQueueMeta>>, PgmqError> {
        let queues = sqlx::query_as!(PGMQueueMeta, "SELECT * from pgmq_list_queues();").fetch_all(&self.connection).await?;
        if queues.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(queues))
        }
    }

    // io methods
    pub async fn set_vt(queue_name: String, vt: u32) -> bool {
        true
    }
    pub async fn send(queue_name: String, message: String) -> bool {
        true
    }
    pub async fn read(queue_name: String) -> String {
        String::from("")
    }
    pub async fn pop(queue_name: String) -> String {
        String::from("")
    }
    pub async fn delete(queue_name: String, msg_id: u32) -> bool {
        true
    }

    // 
    pub async fn archive(queue_name: String, msg_id: u32) -> bool {
        true
    }
}
