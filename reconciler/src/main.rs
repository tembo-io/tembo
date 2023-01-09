use kube::{Client, ResourceExt};
use log::info;
use pgmq::{PGMQueue};
use reconciler::{create_or_update, delete, get_all};
use std::env;
use std::{thread, time};

#[tokio::main]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // // Read connection info from environment variable
    let pg_conn_url = env::var("PG_CONN_URL").expect("PG_CONN_URL must be set");
    let pg_queue_name = env::var("PG_QUEUE_NAME").expect("PG_QUEUE_NAME must be set");

    // Connect to postgres queue
    let queue: PGMQueue = PGMQueue::new(pg_conn_url).await;

    // Infer the runtime environment and try to create a Kubernetes Client
    let client = Client::try_default().await?;

    loop {
        // Read from queue (check for new message)
        let read_msg = match queue.read(&pg_queue_name, Some(&30_u32)).await {
            Some(message) => {
                print!("read_msg: {:?}", message);
                message
            }
            None => {
                thread::sleep(time::Duration::from_secs(1));
                continue;
            }
        };

        // Based on action in message, create, update, delete PostgresCluster
        match serde_json::from_str(&read_msg.message["action"].to_string()).unwrap() {
            Some("create") | Some("update") => {
                let spec = read_msg.message["spec"].clone();

                // create or update PostgresCluster
                create_or_update(client.clone(), "default".to_owned(), spec)
                    .await
                    .expect("error creating or updating PostgresCluster");
            }
            Some("delete") => {
                let name: String =
                    serde_json::from_value(read_msg.message["spec"]["metadata"]["name"].clone())
                        .unwrap();

                // delete PostgresCluster
                delete(client.clone(), "default".to_owned(), name)
                    .await
                    .expect("error deleting PostgresCluster");
            }
            None | _ => println!("action was not in expected format"),
        }

        // TODO(ianstanton) This is here as an example for now. We want to use
        //  this to ensure a PostgresCluster exists before we attempt to delete it.
        // Get all existing PostgresClusters
        let vec = get_all(client.clone(), "default".to_owned());
        for pg in vec.await.iter() {
            println!("found PostgresCluster {}", pg.name_any());
        }
        thread::sleep(time::Duration::from_secs(1));

        // Delete message from queue
        let deleted = queue
            .delete(&pg_queue_name, &read_msg.msg_id)
            .await
            .expect("error deleting message from queue");
        // TODO(ianstanton) Improve logging everywhere
        println!("deleted: {:?}", deleted);
    }
}

fn main() {
    env_logger::init();
    info!("starting");
    run().unwrap();
}
