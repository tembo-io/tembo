use super::SubCommand;
use async_trait::async_trait;
use clap::Args;

#[derive(Args)]
pub struct PublishCommand {}

#[async_trait]
impl SubCommand for PublishCommand {
    async fn execute(&self) -> Result<(), anyhow::Error> {
        println!("trunk publish: not implemented");
        Ok(())
    }
}
