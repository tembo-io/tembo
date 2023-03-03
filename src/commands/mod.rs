use async_trait::async_trait;
pub mod build;
pub mod install;
mod pgx;
pub mod publish;

#[async_trait]
pub trait SubCommand {
    async fn execute(&self) -> Result<(), anyhow::Error>;
}
