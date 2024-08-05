use anyhow::Context;
use log::info;
use pgmq::PGMQueueExt;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::{env, time::Duration};
use tokio::time::interval;

pub fn split_events(events: Message, max_size: usize) -> Vec<Message> {
    let mut result = Vec::new();
    let mut chunk = Vec::new();

    for item in metrics.result.into_iter() {
        if chunk.len() == max_size {
            result.push(DataPlaneMetrics {
                name: metrics.name.clone(),
                result: chunk,
            });
            chunk = Vec::new();
        }
        chunk.push(item);
    }

    if !chunk.is_empty() {
        result.push(DataPlaneMetrics {
            name: metrics.name.clone(),
            result: chunk,
        });
    }

    result
}

pub async fn get_usage(
    dbclient: &Pool<Postgres>,
    start_time: String,
    end_time: String,
) -> Vec<Events> {
    let rows = sqlx::query!(
        "
        SELECT
          organization_id,
          instance_id,
          prompt_tokens,
          completion_tokens
        FROM inference.requests
        WHERE
          completed_at >= $1 AND completed_at < $2
        GROUP BY organization_id, instance_id;
        ",
        start_time,
        end_time
    )
    .fetch_all(dbclient)
    .await?;

    Ok(rows)
}

pub async fn events_reporter() -> Results<()> {
    const BATCH_SIZE: usize = 1000;

    let pg_conn_url = env::var("POSTGRES_QUEUE_CONNECTION")
        .with_context(|| "POSTGRES_QUEUE_CONNECTION must be set")?;

    let queue = PGMQueueExt::new(pg_conn_url, 5).await?;

    // TODO: Need to set this env variable
    let metrics_events_queue =
        env::var("BILLING_EVENTS_QUEUE").expect("BILLING_EVENTS_QUEUE must be set");

    queue.init().await?;
    queue.create(&metrics_events_queue).await?;

    let mut sync_interval = interval(Duration::from_secs(60));

    loop {
        sync_interval.tick().await;

        let events = get_usage('t', "2024-07-01".to_string(), "2024-06-01".to_string());

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
    pub completed_at: String, // Note sure if this is the best type for timestamps
    pub duration_ms: i32,     // We may not need for orb, but never hurts to have more information
    pub model: String,
    pub prompt_tokens: String,
    pub completion_tokens: String,
}
