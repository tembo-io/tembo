use std::time::Duration;

#[derive(Debug)]
pub struct TaskConfig {
    pub max_errors: u32,
    pub health_timeout: Duration,
    pub retry_base_delay: Duration,
    pub retry_max_delay: Duration,
    pub retry_max_attempts: u32,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            max_errors: 3,
            health_timeout: Duration::from_secs(120),
            retry_base_delay: Duration::from_secs(1),
            retry_max_delay: Duration::from_secs(30),
            retry_max_attempts: 10,
        }
    }
}
