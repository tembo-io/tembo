use super::SubCommand;
use async_trait::async_trait;
use clap::Args;

#[derive(Args)]
pub struct InstallCommand {
    #[arg(long = "pg-config", short = 'p', default_value = "")]
    pg_config: String,
}

#[async_trait]
impl SubCommand for InstallCommand {
    async fn execute(&self) -> Result<(), anyhow::Error> {
        let mut pg_config = String::new();
        if self.pg_config == "" {
            pg_config = which::which("pg_config")?
                .into_os_string()
                .into_string()
                .unwrap();
        } else {
            // find pg_config in path
            pg_config = self.pg_config.clone();
        }
        println!("Using pg_config: {}", pg_config);
        Ok(())
    }
}
