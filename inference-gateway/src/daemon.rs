//! Custom entrypoint for background running services

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let background_threads: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(Vec::new()));
    
    
}