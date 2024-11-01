use super::config::TaskConfig;
use crate::health::TaskHealth;
use crate::monitoring::CustomMetrics;
use exponential_backoff::Backoff;
use log::{error, info, warn};
use opentelemetry::KeyValue;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};

pub async fn run_background_task<F, Fut, E>(
    name: &str,
    mut shutdown_rx: broadcast::Receiver<()>,
    task_health: Arc<RwLock<HashMap<String, TaskHealth>>>,
    custom_metrics: Arc<CustomMetrics>,
    config: TaskConfig,
    task_fn: F,
) where
    F: Fn(Arc<CustomMetrics>) -> Fut,
    Fut: Future<Output = Result<(), E>>,
    E: std::error::Error,
{
    loop {
        let mut consecutive_errors = 0;

        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("{} received shutdown signal", name);
                break;
            }
            _ = async {
                // Create new backoff iterator for each attempt series
                let backoff = Backoff::new(
                    config.retry_max_attempts,
                    config.retry_base_delay,
                    config.retry_max_delay,
                );

                for duration in backoff {
                    match task_fn(custom_metrics.clone()).await {
                        Ok(_) => {
                            let mut health = task_health.write().await;
                            if let Some(task_health) = health.get_mut(name) {
                                task_health.consecutive_errors = 0;
                                task_health.is_healthy = true;
                                task_health.last_heartbeat = Instant::now();
                            }
                            consecutive_errors = 0;
                            break;
                        }
                        Err(err) => {
                            consecutive_errors += 1;

                            let mut health = task_health.write().await;
                            if let Some(task_health) = health.get_mut(name) {
                                task_health.consecutive_errors = consecutive_errors;
                                task_health.is_healthy = false;
                            }

                            custom_metrics.conductor_errors.add(
                                &opentelemetry::Context::current(),
                                1,
                                &[KeyValue::new("task", name.to_string())],
                            );

                            error!("error in {}: {:?}", name, err);

                            if consecutive_errors >= config.max_errors {
                                error!("{} reached max consecutive errors", name);
                                return;
                            }

                            match duration {
                                Some(delay) => {
                                    warn!("{} backing off for {:?}", name, delay);
                                    tokio::time::sleep(delay).await;
                                }
                                None => {
                                    error!("{} exceeded maximum retry attempts", name);
                                    return;
                                }
                            }
                        }
                    }
                }

                // After the for loop completes, wait before starting next iteration
                tokio::time::sleep(Duration::from_secs(1)).await;
            } => {}
        }
    }
}
