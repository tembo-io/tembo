use super::SubCommand;
use async_trait::async_trait;
use clap::Args;

#[derive(Args)]
pub struct InstallCommand {}

#[async_trait]
impl SubCommand for InstallCommand {
    async fn execute(&self) -> Result<(), anyhow::Error> {
        println!("trunk install: not implemented");
        Ok(())
    }
}
