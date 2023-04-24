use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use tokio_task_manager::Task;
use futures_util::stream::StreamExt;

/// Used to stop container when dropped, relies on using [tokio_task_manager::TaskManager::wait]
/// to ensure `Drop` will run to completion
pub struct ReclaimableContainer<'a> {
    id: &'a str,
    docker: Docker,
    task: Task,
}

impl<'a> ReclaimableContainer<'a> {
    #[must_use]
    pub fn new(name: &'a str, docker: &Docker, task: Task) -> Self {
        Self {
            id: name,
            docker: docker.clone(),
            task,
        }
    }
}

impl<'a> Drop for ReclaimableContainer<'a> {
    fn drop(&mut self) {
        let docker = self.docker.clone();
        let id = self.id.to_string();
        let handle = tokio::runtime::Handle::current();
        let mut task = self.task.clone();
        handle.spawn(async move {
            println!("Stopping {id}");
            docker
                .stop_container(&id, None)
                .await
                .expect("error stopping container");
            println!("Stopped {id}");
            task.wait().await;
        });
    }
}

pub async fn exec_in_container(docker: Docker, container_id: &str, command: Vec<&str>) -> Result<String, anyhow::Error> {

    let config = CreateExecOptions {
        cmd: Some(command),
        attach_stdout: Some(true),
        ..Default::default()
    };

    let exec = docker.create_exec(container_id, config).await?;
    let start_exec_options = Some(StartExecOptions {
        detach: false,
        ..StartExecOptions::default()
    });
    let log_output = docker.start_exec(&exec.id, start_exec_options);
    let mut start_exec_result = log_output.await?;

    let mut total_output = String::new();
    match start_exec_result {
        StartExecResults::Attached { output, .. } => {
            let mut output = output
                .map(|result| {
                    match result {
                        Ok(log_output) => {
                            println!("{}", log_output.to_string());
                            total_output.push_str(log_output.to_string().as_str());
                        },
                        Err(error) => eprintln!("Error while reading log output: {}", error),
                    }
                })
                .fuse();

            // Run the output stream to completion.
            while output.next().await.is_some() {}
        },
        StartExecResults::Detached => {
            println!("Exec started in detached mode");
        }

    }
    Ok::<_, anyhow::Error>(total_output)
}