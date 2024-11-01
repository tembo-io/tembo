use crate::tasks::config::TaskConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone)]
pub struct TaskHealth {
    pub name: String,
    pub last_heartbeat: Instant,
    pub consecutive_errors: u32,
    pub is_healthy: bool,
}

impl TaskHealth {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            last_heartbeat: Instant::now(),
            consecutive_errors: 0,
            is_healthy: true,
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub task_health: Arc<RwLock<HashMap<String, TaskHealth>>>,
    pub shutdown_tx: broadcast::Sender<()>,
    pub config: TaskConfig,
}
