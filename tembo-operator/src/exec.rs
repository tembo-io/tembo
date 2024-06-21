use k8s_openapi::api::core::v1::Pod;
use kube::{api::Api, client::Client, core::subresource::AttachParams};
use tokio::io::AsyncReadExt;

use crate::Error;
use tracing::{debug, error, warn};

#[derive(Debug)]
pub struct ExecOutput {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub success: bool,
}

impl ExecOutput {
    pub fn new(stdout: Option<String>, stderr: Option<String>, success: bool) -> Self {
        Self {
            stdout,
            stderr,
            success,
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
            max_stdin_buf_size: Some(10240),
            max_stdout_buf_size: Some(10240),
            max_stderr_buf_size: Some(10240),
        };

        let mut attached_process = self
            .pods_api
            .exec(self.pod_name.as_str(), command, &attach_params)
            .await?;

        let result_stdout = match attached_process.stdout() {
            None => {
                warn!("No stdout from exec to pod: {:?}", self.pod_name);
                String::new()
            }
            Some(mut stdout_reader) => {
                let mut result_stdout = String::new();
                stdout_reader
                    .read_to_string(&mut result_stdout)
                    .await
                    .unwrap_or_default();
                result_stdout
            }
        };

        let result_stderr = match attached_process.stderr() {
            None => {
                warn!("No stderr from exec to pod: {:?}", self.pod_name);
                String::new()
            }
            Some(mut stderr_reader) => {
                let mut result_stderr = String::new();
                stderr_reader
                    .read_to_string(&mut result_stderr)
                    .await
                    .unwrap_or_default();
                result_stderr
            }
        };

        let status = match attached_process.take_status() {
            None => {
                return Err(Error::KubeExecError(format!(
                    "Error executing command: {:?} on pod: {:?}. Failed to find command status.",
                    command, self.pod_name
                )));
            }
            Some(status) => status.await.unwrap_or_default(),
        };
        // https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#spec-and-status

        let success = match status.status.expect("no status reported").as_str() {
            "Success" => true,
            "Failure" => {
                let output = format!(
                    "stdout:\n{}\nstderr:\n{}",
                    result_stdout.clone(),
                    result_stderr.clone()
                );
                warn!(
                    "Error executing command on pod: {:?}. response: {:?}",
                    self.pod_name, output
                );

                if let Some(reason) = &status.reason {
                    warn!(
                        "Reason for failed kube exec: {reason}, code {:?}",
                        status.code
                    );
                }
                debug!("Failed command: {:?}", command);
                false
            }
            // This is never supposed to happen because status is supposed to only be
            // Success or Failure based on how the Kube API works
            _ => {
                error!(
                    "Undefined response from kube API when exec to pod: {:?}",
                    self.pod_name
                );
                return Err(Error::KubeExecError(format!(
                    "Error executing command: {:?} on pod: {:?}.",
                    command, self.pod_name
                )));
            }
        };
        Ok(ExecOutput::new(
            Some(result_stdout),
            Some(result_stderr),
            success,
        ))
    }
}
