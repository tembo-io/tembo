use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use log::info;
use pgmq::PGMQueueExt;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::{env, time::Duration};
use tokio::time::interval;
use uuid::Uuid;

use crate::db::connect;
use crate::errors::DatabaseError;

pub fn split_events(events: Vec<Events>, max_size: usize) -> Vec<Message> {
    events
        .chunks(max_size)
        .map(|chunk| Message {
            id: Uuid::new_v4().to_string(),
            message: chunk.to_vec(),
        })
        .collect()
}

pub async fn get_usage(
    dbclient: &Pool<Postgres>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<Vec<Events>, DatabaseError> {
    let rows = sqlx::query_as!(
        UsageData,
        r#"
        SELECT
          organization_id,
          instance_id,
          duration_ms,
          prompt_tokens,
          completion_tokens,
          model,
          completed_at
        FROM inference.requests
        WHERE
          completed_at >= $1 AND completed_at < $2
        "#,
        start_time,
        end_time
    )
    .fetch_all(dbclient)
    .await?;

    Ok(rows_to_events(rows))
}

pub fn rows_to_events(rows: Vec<UsageData>) -> Vec<Events> {
    rows.into_iter()
        .map(|row| Events {
            idempotency_key: Uuid::new_v4().to_string(),
            organization_id: row.organization_id,
            instance_id: row.instance_id,
            payload: Payload {
                completed_at: row.completed_at.to_string(),
                duration_ms: row.duration_ms,
                model: row.model,
                prompt_tokens: row.prompt_tokens.to_string(),
                completion_tokens: row.completion_tokens.to_string(),
            },
        })
        .collect()
}

pub async fn events_reporter() -> Result<()> {
    const BATCH_SIZE: usize = 1000;

    let pg_conn_url = env::var("POSTGRES_QUEUE_CONNECTION")
        .with_context(|| "POSTGRES_QUEUE_CONNECTION must be set")?;
    let dbclient = connect(&pg_conn_url, 5).await?;

    let queue = PGMQueueExt::new(pg_conn_url, 5).await?;

    // TODO: Need to set this env variable
    let metrics_events_queue =
        env::var("BILLING_EVENTS_QUEUE").expect("BILLING_EVENTS_QUEUE must be set");

    queue.init().await?;
    queue.create(&metrics_events_queue).await?;

    let mut sync_interval = interval(Duration::from_secs(3600));

    loop {
        sync_interval.tick().await;

        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::hours(1);
        // TODO: Get the correct postgres connection URL
        let events = get_usage(&dbclient, start_time, end_time).await?;

        let metrics_to_send = split_events(events, BATCH_SIZE);
        let batches = metrics_to_send.len();

        info!(
            "Split metrics into {} chunks, each with {} results",
            batches, BATCH_SIZE
        );

        let mut i = 1;
        for event in &metrics_to_send {
            queue.send(&metrics_events_queue, event).await?;
            info!("Enqueued batch {}/{} to PGMQ", i, batches);
            i += 1;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: String,
    pub message: Vec<Events>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Events {
    pub idempotency_key: String,
    pub organization_id: String,
    pub instance_id: String,
    pub payload: Payload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Payload {
    pub completed_at: String,
    pub duration_ms: i32,
    pub model: String,
    pub prompt_tokens: String,
    pub completion_tokens: String,
}

pub struct UsageData {
    organization_id: String,
    instance_id: String,
    duration_ms: i32,
    prompt_tokens: i32,
    completion_tokens: i32,
    model: String,
    completed_at: DateTime<Utc>,
}

//TODO: Add unit tests
