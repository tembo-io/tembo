use async_trait::async_trait;
use tokio_task_manager::Task;

pub mod build;
pub mod install;
mod pgx;
pub mod publish;

#[async_trait]
pub trait SubCommand {
    async fn execute(&self, task: Task) -> Result<(), anyhow::Error>;
}
