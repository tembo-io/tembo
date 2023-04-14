use bollard::Docker;
use tokio_task_manager::Task;

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
