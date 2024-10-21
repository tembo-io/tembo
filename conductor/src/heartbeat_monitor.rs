use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("now() is not later than UNIX_EPOCH")
        .as_secs()
}

pub struct HeartbeatMonitor {
    shared_heartbeat: Arc<AtomicU64>,
    update_interval: Duration,
}

#[derive(Clone)]
pub struct HeartbeatUpdater {
    shared_heartbeat: Arc<AtomicU64>,
}

/// Initializes and returns both a [`HeartbeatMonitor`] and [`HeartbeatUpdater`].
pub fn start(expected_update_interval: Duration) -> (HeartbeatMonitor, HeartbeatUpdater) {
    let heartbeat = Arc::new(AtomicU64::new(current_timestamp()));

    let heartbeat_monitor = HeartbeatMonitor {
        shared_heartbeat: heartbeat.clone(),
        update_interval: expected_update_interval,
    };
    let heartbeat_updater = HeartbeatUpdater {
        shared_heartbeat: heartbeat,
    };

    (heartbeat_monitor, heartbeat_updater)
}

impl HeartbeatMonitor {
    /// Checks if the heartbeat is still active
    ///
    /// # Returns true if the heartbeat has been updated within the expected time frame, false if the heartbeat has not been updated within twice the expected timeout duration
    pub fn is_heartbeat_active(&self) -> bool {
        let last_update = self.shared_heartbeat.load(Ordering::Relaxed);
        let current_time = current_timestamp();

        if current_time >= last_update {
            let elapsed = Duration::from_secs(current_time - last_update);
            elapsed < self.update_interval * 2
        } else {
            // System time went backwards or clock drift, consider the heartbeat stale
            false
        }
    }
}

impl HeartbeatUpdater {
    pub fn update_heartbeat(&self) {
        self.shared_heartbeat
            .store(current_timestamp(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::heartbeat_monitor;

    #[tokio::test]
    async fn check_heartbeat_monitor() {
        let (monitor, updater) = heartbeat_monitor::start(Duration::from_secs(1));

        // Is alive since there's been an update in the last second
        assert!(monitor.is_heartbeat_active());

        tokio::time::sleep(Duration::from_secs(4)).await;

        assert_eq!(monitor.is_heartbeat_active(), false);
        updater.update_heartbeat();

        assert!(monitor.is_heartbeat_active());
    }
}
