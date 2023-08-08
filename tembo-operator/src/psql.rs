use crate::{Context, Error};

use kube::Client;
use std::sync::Arc;


use crate::exec::ExecCommand;

pub struct PsqlOutput {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    // k8s_openapi::apimachinery::pkg::apis::meta::v1::Status
    pub success: bool,
}

impl PsqlOutput {
    pub fn new(stdout: Option<String>, stderr: Option<String>, success: bool) -> Self {
        Self {
            stdout,
            stderr,
            success,
        }
    }
}

pub struct PsqlCommand {
    pod_name: String,
    namespace: String,
    database: String,
    command: String,
    client: Client,
}

impl PsqlCommand {
    pub fn new(
        pod_name: String,
        namespace: String,
        command: String,
        database: String,
        context: Arc<Context>,
    ) -> Self {
        Self {
            pod_name,
            namespace,
            database,
            command,
            client: context.client.clone(),
        }
    }

    pub async fn execute(&self) -> Result<PsqlOutput, Error> {
        let psql_command = vec![
            String::from("psql"),
            self.database.clone(),
            String::from("-c"),
            self.command.clone(),
        ];
        let command = ExecCommand::new(self.pod_name.clone(), self.namespace.clone(), self.client.clone());
        let output = command.execute(&psql_command).await?;

        Ok(PsqlOutput::new(output.stdout, output.stderr, output.success))
    }
}
