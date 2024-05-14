use crate::Context;

use kube::{runtime::controller::Action, Client};
use std::{sync::Arc, time::Duration};
use tracing::warn;

use crate::exec::ExecCommand;

#[derive(Debug)]
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

    // Get a field from the stdout by index
    pub fn get_field(&self, index: usize) -> Option<String> {
        self.stdout.as_ref().and_then(|output| {
            let lines: Vec<&str> = output.lines().collect();

            lines.get(2).map(|line| {
                line.split('|')
                    .map(str::trim)
                    .nth(index)
                    .unwrap_or("")
                    .to_string()
            })
        })
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

    pub async fn execute(&self) -> Result<PsqlOutput, Action> {
        let psql_command = vec![
            String::from("psql"),
            format!(
                "postgres://?dbname={}&application_name=tembo-system",
                self.database.clone()
            ),
            String::from("-c"),
            self.command.clone(),
        ];
        let command = ExecCommand::new(
            self.pod_name.clone(),
            self.namespace.clone(),
            self.client.clone(),
        );
        let output = match command.execute(&psql_command).await {
            Ok(output) => output,
            Err(e) => {
                warn!(
                    "{}: Failed to kubectl exec a psql command: {:?}",
                    self.namespace, e
                );
                return Err(Action::requeue(Duration::from_secs(10)));
            }
        };

        if !output.success
            && output.stderr.clone().is_some()
            && output
                .stderr
                .clone()
                .unwrap()
                .contains("the database system is shutting down")
        {
            warn!(
                "Failed to execute psql command because DB is shutting down. Requeueing. {}",
                self.namespace
            );
            return Err(Action::requeue(Duration::from_secs(10)));
        }

        Ok(PsqlOutput::new(
            output.stdout,
            output.stderr,
            output.success,
        ))
    }
}
