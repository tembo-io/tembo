use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, TimeZone, Timelike, Utc};
use log::info;
use pgmq::PGMQueueExt;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Pool, Postgres};
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
        model,
        MAX(completed_at) AS completed_at,
        SUM(prompt_tokens) AS prompt_tokens,
        SUM(completion_tokens) AS completion_tokens
    FROM
        inference.requests
    WHERE
        completed_at >= $1
        AND completed_at <= $2
    GROUP BY
        organization_id,
        instance_id,
        model
        "#,
        start_time,
        end_time
    )
    .fetch_all(dbclient)
    .await?;

    Ok(rows_to_events(rows))
}

fn rows_to_events(rows: Vec<UsageData>) -> Vec<Events> {
    rows.into_iter()
        .map(|row| {
            // Parse the completed_at string into a DateTime<Utc> and convert to hour
            let completed_at = row
                .completed_at
                .unwrap()
                .with_timezone(&Utc)
                .format("%Y%m%d%H")
                .to_string();

            Events {
                idempotency_key: format!("{}-{}-{}", row.instance_id, row.model, completed_at),
                organization_id: row.organization_id,
                instance_id: row.instance_id,
                payload: Payload {
                    completed_at: row.completed_at.unwrap_or_default().to_string(),
                    model: row.model,
                    prompt_tokens: row.prompt_tokens.unwrap_or(0).to_string(),
                    completion_tokens: row.completion_tokens.unwrap_or(0).to_string(),
                },
            }
        })
        .collect()
}

async fn get_reporter_watermark(conn: &PgPool) -> Result<Option<ReporterWatermark>> {
    sqlx::query_as!(
        ReporterWatermark,
        "SELECT last_reported_at FROM billing.reporter_watermark LIMIT 1"
    )
    .fetch_optional(conn)
    .await
    .map_err(Into::into)
}

fn start_of_the_hour(datetime: DateTime<Utc>) -> DateTime<Utc> {
    // Safe unwrap since, according to chrono docs, Utc will never have double mappings
    Utc.with_ymd_and_hms(
        datetime.year(),
        datetime.month(),
        datetime.day(),
        datetime.hour(),
        0,
        0,
    )
    .unwrap()
}

fn get_hourly_chunks(
    last_reported_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let last_complete_hour = start_of_the_hour(now);

    let mut chunks = Vec::new();
    let mut chunk_start = start_of_the_hour(last_reported_at);

    while chunk_start < last_complete_hour {
        let chunk_end = chunk_start + ChronoDuration::hours(1) - ChronoDuration::nanoseconds(1);
        chunks.push((chunk_start, chunk_end));
        chunk_start = chunk_start + ChronoDuration::hours(1);
    }

    chunks
}

pub async fn run_events_reporter(pool: PgPool) -> Result<()> {
    // Run once per hour
    const SYNC_PERIOD: Duration = Duration::from_secs(60 * 60);

    // let queue = PGMQueueExt::new(pg_conn_url, 5).await?;

    // TODO: Need to set this env variable
    let metrics_events_queue =
        env::var("BILLING_EVENTS_QUEUE").expect("BILLING_EVENTS_QUEUE must be set");

    queue.init().await?;
    queue.create(&metrics_events_queue).await?;

    let mut sync_interval = interval(SYNC_PERIOD);

    loop {
        sync_interval.tick().await;

        let last_reported_at = get_reporter_watermark(&dbclient)
            .await?
            .map(|watermark| watermark.last_reported_at)
            .unwrap_or(Utc::now() - Duration::from_secs(60 * 60));
        let now = Utc::now();

        let chunks = get_hourly_chunks(last_reported_at, now);

        for (start_time, end_time) in chunks {
            enqueue_event(
                &dbclient,
                &queue,
                &metrics_events_queue,
                start_time,
                end_time,
            )
            .await?;
        }
    }
}

async fn enqueue_event(
    pool: &Pool<Postgres>,
    queue: &PGMQueueExt,
    metrics_events_queue: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<(), anyhow::Error> {
    const BATCH_SIZE: usize = 1000;

    let events = get_usage(pool, start_time, end_time).await?;
    let metrics_to_send = split_events(events, BATCH_SIZE);
    let batches = metrics_to_send.len();
    info!(
        "Split metrics into {} chunks, each with {} results",
        batches, BATCH_SIZE
    );
    let mut i = 1;

    for event in &metrics_to_send {
        queue.send(metrics_events_queue, event).await?;
        info!(
            "Enqueued batch {}/{} for {} to PGMQ",
            i,
            batches,
            start_time.format("%Y-%m-%d %H:%M:%S %Z")
        );
        i += 1;
    }

    Ok(())
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
    pub model: String,
    pub prompt_tokens: String,
    pub completion_tokens: String,
}

struct UsageData {
    organization_id: String,
    instance_id: String,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    model: String,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow, Debug)]
struct ReporterWatermark {
    last_reported_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, FixedOffset, TimeZone, Timelike, Utc};

    use crate::events_reporter::start_of_the_hour;

    use super::get_hourly_chunks;

    #[test]
    fn test_start_of_hour_middle_of_hour() {
        // Middle of the hour
        let input = Utc.with_ymd_and_hms(2023, 5, 10, 15, 30, 45).unwrap();
        let expected = Utc.with_ymd_and_hms(2023, 5, 10, 15, 0, 0).unwrap();
        assert_eq!(start_of_the_hour(input), expected);

        // Already at start of the hour
        let input = Utc.with_ymd_and_hms(2023, 5, 10, 15, 0, 0).unwrap();
        let expected = Utc.with_ymd_and_hms(2023, 5, 10, 15, 0, 0).unwrap();
        assert_eq!(start_of_the_hour(input), expected);

        // End of the hour
        let input = Utc.with_ymd_and_hms(2023, 5, 10, 15, 59, 59).unwrap();
        let expected = Utc.with_ymd_and_hms(2023, 5, 10, 15, 0, 0).unwrap();
        assert_eq!(start_of_the_hour(input), expected);

        // Midnight
        let input = Utc.with_ymd_and_hms(2023, 5, 10, 0, 30, 0).unwrap();
        let expected = Utc.with_ymd_and_hms(2023, 5, 10, 0, 0, 0).unwrap();
        assert_eq!(start_of_the_hour(input), expected);

        // Different timezone (EST)
        let est = FixedOffset::west_opt(5 * 3600).unwrap(); // UTC-5
        let input_est = est.with_ymd_and_hms(2023, 5, 10, 10, 30, 0).unwrap();
        let input_utc: DateTime<Utc> = input_est.with_timezone(&Utc);
        let expected = Utc.with_ymd_and_hms(2023, 5, 10, 15, 0, 0).unwrap();
        assert_eq!(start_of_the_hour(input_utc), expected);
    }

    #[test]
    fn test_generate_hourly_chunks() {
        // Mock current time as 2023-05-10 15:40:00 UTC
        let now = Utc.with_ymd_and_hms(2023, 5, 10, 15, 40, 0).unwrap();

        // Set last_reported_at to 2023-05-10 12:20:00 UTC
        let last_reported_at = Utc.with_ymd_and_hms(2023, 5, 10, 12, 20, 0).unwrap();

        // Generate chunks
        let chunks = get_hourly_chunks(last_reported_at, now);

        // Expected chunks
        let expected_chunks = vec![
            // 2023-05-10 12:00:00 UTC to 2023-05-10 12:59:59 UTC
            (
                Utc.with_ymd_and_hms(2023, 5, 10, 12, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2023, 5, 10, 12, 59, 59)
                    .unwrap()
                    .with_nanosecond(999999999)
                    .unwrap(),
            ),
            // 2023-05-10 13:00:00 UTC to 2023-05-10 13:59:59 UTC
            (
                Utc.with_ymd_and_hms(2023, 5, 10, 13, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2023, 5, 10, 13, 59, 59)
                    .unwrap()
                    .with_nanosecond(999999999)
                    .unwrap(),
            ),
            // 2023-05-10 14:00:00 UTC to 2023-05-10 14:59:59 UTC
            (
                Utc.with_ymd_and_hms(2023, 5, 10, 14, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2023, 5, 10, 14, 59, 59)
                    .unwrap()
                    .with_nanosecond(999999999)
                    .unwrap(),
            ),
        ];

        assert_eq!(chunks, expected_chunks);
    }
}
