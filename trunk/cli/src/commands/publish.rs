use super::SubCommand;
use async_trait::async_trait;
use clap::Args;
use tokio_task_manager::Task;

#[derive(Args)]
pub struct PublishCommand {}

#[async_trait]
impl SubCommand for PublishCommand {
    async fn execute(&self, _task: Task) -> Result<(), anyhow::Error> {
        println!("trunk publish: not implemented");
        Ok(())
    }
}
