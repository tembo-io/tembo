use k8s_openapi::{api::core::v1::Pod, apimachinery::pkg::apis::meta::v1::Status};
use kube::{api::Api, client::Client, core::subresource::AttachParams};
use tokio::io::AsyncReadExt;

use crate::Error;
use tracing::error;

#[derive(Debug)]
pub struct ExecOutput {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub status: Option<Status>,
}

impl ExecOutput {
    pub fn new(stdout: Option<String>, stderr: Option<String>, status: Option<Status>) -> Self {
        Self {
            stdout,
            stderr,
            status,
        }
    }
}

pub struct ExecCommand {
    pods_api: Api<Pod>,
    pod_name: String,
}

impl ExecCommand {
    pub fn new(pod_name: String, namespace: String, client: Client) -> Self {
        let pods_api: Api<Pod> = Api::namespaced(client, &namespace);
        Self { pod_name, pods_api }
    }

    pub async fn execute(&self, command: &[String]) -> Result<ExecOutput, Error> {
        let attach_params = AttachParams {
            container: Some("postgres".to_string()),
            tty: false,
            stdin: true,
            stdout: true,
            stderr: true,
            max_stdin_buf_size: Some(1024),
            max_stdout_buf_size: Some(1024),
            max_stderr_buf_size: Some(1024),
        };

        let mut attached_process = self
            .pods_api
            .exec(self.pod_name.as_str(), command, &attach_params)
            .await?;

        let mut stdout_reader = attached_process.stdout().unwrap();
        let mut result_stdout = String::new();
        stdout_reader.read_to_string(&mut result_stdout).await.unwrap();

        let mut stderr_reader = attached_process.stderr().unwrap();
        let mut result_stderr = String::new();
        stderr_reader.read_to_string(&mut result_stderr).await.unwrap();


        let status = attached_process.take_status().unwrap().await.unwrap();
        // https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#spec-and-status
        let response = ExecOutput::new(Some(result_stdout), Some(result_stderr), Some(status.clone()));

        match status.status.expect("no status reported").as_str() {
            "Success" => Ok(response),
            "Failure" => {
                error!("Error executing command: {:?}. response: {:?}", command, response);
                Err(Error::KubeExecError(format!(
                    "Error executing command: {:?}. response: {:?}",
                    command, response
                )))
            }
            _ => {
                error!(
                    "Undefined response from kube API {:?}, command: {:?}",
                    response, command
                );
                Err(Error::KubeExecError(format!(
                    "Error executing command: {:?}. response: {:?}",
                    command, response
                )))
            }
        }
    }
}
