use k8s_openapi::{api::core::v1::Pod, apimachinery::pkg::apis::meta::v1::Status};
use kube::{api::Api, client::Client, core::subresource::AttachParams};
use tokio::io::AsyncReadExt;

pub struct PsqlOutput {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    // k8s_openapi::apimachinery::pkg::apis::meta::v1::Status
    pub status: Option<Status>,
}

impl PsqlOutput {
    pub fn new(stdout: Option<String>, stderr: Option<String>, status: Option<Status>) -> Self {
        Self {
            stdout,
            stderr,
            status,
        }
    }
}

pub struct PsqlCommand {
    pods_api: Api<Pod>,
    pod_name: String,
    database: String,
    command: String,
}

impl PsqlCommand {
    pub fn new(
        pod_name: String,
        namespace: String,
        command: String,
        database: String,
        client: Client,
    ) -> Self {
        let pods_api: Api<Pod> = Api::namespaced(client, &namespace);
        Self {
            pod_name,
            pods_api,
            database,
            command,
        }
    }

    pub async fn execute(&self) -> Result<PsqlOutput, kube::Error> {
        let attach_params = AttachParams {
            container: None,
            tty: false,
            stdin: true,
            stdout: true,
            stderr: true,
            max_stdin_buf_size: Some(1024),
            max_stdout_buf_size: Some(1024),
            max_stderr_buf_size: Some(1024),
        };

        let psql_command = vec![
            String::from("psql"),
            self.database.clone(),
            String::from("-c"),
            self.command.clone(),
        ];

        let mut attached_process = self
            .pods_api
            .exec(self.pod_name.as_str(), &psql_command, &attach_params)
            .await
            .unwrap();

        // https://docs.rs/tokio/latest/tokio/io/trait.AsyncReadExt.html#method.read_to_string
        // Since waiting for EOF to be reached, a join is not needed and the attached_process will
        // be completed before returning the value

        // STDOUT
        let mut stdout_reader = attached_process.stdout().unwrap();
        let mut result_stdout = String::new();
        stdout_reader.read_to_string(&mut result_stdout).await.unwrap();

        // STDERR
        let mut stderr_reader = attached_process.stderr().unwrap();
        let mut result_stderr = String::new();
        stderr_reader.read_to_string(&mut result_stderr).await.unwrap();

        // Status
        // https://docs.rs/k8s-openapi/latest/k8s_openapi/apimachinery/pkg/apis/meta/v1/struct.Status.html
        let status = attached_process.take_status().unwrap().await.unwrap();

        return Ok(PsqlOutput::new(
            Some(result_stdout),
            Some(result_stderr),
            Some(status),
        ));
    }
}
