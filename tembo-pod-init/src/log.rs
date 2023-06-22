use std::error::Error;
use tracing::Level;
use tracing_log::LogTracer;
use tracing_subscriber::FmtSubscriber;

use crate::config::Config;

pub fn init(config: &Config) -> Result<(), Box<dyn Error>> {
    LogTracer::init()?;

    let log_level = match config.log_level.to_lowercase().as_str() {
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
