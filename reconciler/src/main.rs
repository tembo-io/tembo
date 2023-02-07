use kube::{Client, ResourceExt};
use log::{info, warn};
use pgmq::{Message, PGMQueue};
use reconciler::{
    create_ing_route_tcp, create_namespace, create_or_update, delete, delete_namespace,
    generate_spec, get_all, get_pg_conn,
};
use std::env;
use std::{thread, time};

use reconciler::types;

#[tokio::main]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // // Read connection info from environment variable
    let pg_conn_url = env::var("PG_CONN_URL").expect("PG_CONN_URL must be set");
    let control_plane_events_queue =
        env::var("CONTROL_PLANE_EVENTS_QUEUE").expect("CONTROL_PLANE_EVENTS_QUEUE must be set");
    let data_plane_events_queue =
        env::var("DATA_PLANE_EVENTS_QUEUE").expect("DATA_PLANE_EVENTS_QUEUE must be set");

    // Connect to pgmq
    let queue: PGMQueue = PGMQueue::new(pg_conn_url).await?;

    // Create queues if they do not exist
    queue.create(&control_plane_events_queue).await?;
    queue.create(&data_plane_events_queue).await?;

    // Infer the runtime environment and try to create a Kubernetes Client
    let client = Client::try_default().await?;

    loop {
        // Read from queue (check for new message)
        let read_msg: Message = match queue
            .read(&control_plane_events_queue, Some(&30_i32))
            .await?
        {
            Some(message) => {
                info!("read_msg: {:?}", message);
                message
            }
            None => {
                thread::sleep(time::Duration::from_secs(1));
                continue;
            }
        };
        let data_plane_id: String =
            serde_json::from_value(read_msg.message["data_plane_id"].clone()).unwrap();
        let event_id: String =
            serde_json::from_value(read_msg.message["event_id"].clone()).unwrap();

        // Based on message_type in message, create, update, delete PostgresCluster
        match serde_json::from_str(&read_msg.message["message_type"].to_string()).unwrap() {
            Some("SnapShot") => {
                info!("Doing nothing for now")
            }
            Some("Create") | Some("Update") => {
                let crud_event_body =
                    serde_json::from_value::<types::EventBody>(read_msg.message["body"].clone())
                        .expect("error parsing body");
                // create namespace if it does not exist
                let namespace: String = crud_event_body.resource_name.clone();

                create_namespace(client.clone(), namespace.clone())
                    .await
                    .expect("error creating namespace");

                // create IngressRouteTCP
                create_ing_route_tcp(client.clone(), namespace.clone())
                    .await
                    .expect("error creating IngressRouteTCP");

                // generate PostgresCluster spec based on values in body
                let spec = generate_spec(&crud_event_body).await;

                // create or update PostgresCluster
                create_or_update(client.clone(), namespace.clone(), spec)
                    .await
                    .expect("error creating or updating PostgresCluster");
                // get connection string values from secret
                let connection_string = get_pg_conn(client.clone(), namespace.clone())
                    .await
                    .expect("error getting secret");

                // report state
                let msg = types::StateToControlPlane {
                    data_plane_id,
                    event_id,
                    state: types::State {
                        connection: Some(connection_string),
                        status: types::Status::Up,
                    },
                };
                let msg_id = queue.send(&data_plane_events_queue, &msg).await?;
                info!("msg_id: {:?}", msg_id);
            }
            Some("Delete") => {
                let name: String =
                    serde_json::from_value(read_msg.message["body"]["resource_name"].clone())
                        .unwrap();

                // delete PostgresCluster
                delete(client.clone(), name.clone(), name.clone())
                    .await
                    .expect("error deleting PostgresCluster");

                // delete namespace
                delete_namespace(client.clone(), name.clone())
                    .await
                    .expect("error deleting namespace");

                // report state
                let msg = types::StateToControlPlane {
                    data_plane_id,
                    event_id,
                    state: types::State {
                        connection: None,
                        status: types::Status::Deleted,
                    },
                };
                let msg_id = queue.send(&data_plane_events_queue, &msg).await?;
                info!("msg_id: {:?}", msg_id);
            }
            None | _ => warn!("action was not in expected format"),
        }

        // TODO(ianstanton) This is here as an example for now. We want to use
        //  this to ensure a PostgresCluster exists before we attempt to delete it.
        // Get all existing PostgresClusters
        let vec = get_all(client.clone(), "default".to_owned());
        for pg in vec.await.iter() {
            info!("found PostgresCluster {}", pg.name_any());
        }
        thread::sleep(time::Duration::from_secs(1));

        // archive message from queue
        let archived = queue
            .archive(&control_plane_events_queue, &read_msg.msg_id)
            .await
            .expect("error archiving message from queue");
        // TODO(ianstanton) Improve logging everywhere
        info!("archived: {:?}", archived);
    }
}

fn main() {
    env_logger::init();
    info!("starting");
    run().unwrap();
}
