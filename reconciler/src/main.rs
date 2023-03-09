use kube::{Client, ResourceExt};
use log::{info, warn};
use pgmq::{Message, PGMQueue};
use reconciler::types;
use reconciler::{
    create_ing_route_tcp, create_metrics_ingress, create_namespace, create_or_update, delete,
    delete_namespace, generate_spec, get_all, get_pg_conn,
};
use std::env;
use std::{thread, time};
use types::{CRUDevent, Event};

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
        // messages that dont fit a CRUDevent will error
        let read_msg = queue
            .read::<CRUDevent>(&control_plane_events_queue, Some(&30_i32))
            .await?;
        let read_msg: Message<CRUDevent> = match read_msg {
            Some(message) => {
                info!("read_msg: {:?}", message);
                message
            }
            None => {
                thread::sleep(time::Duration::from_secs(1));
                continue;
            }
        };

        // TODO: recycld messages should get archived, logged, alerted
        // if read_msg.read_ct >=2 {
        //     warn!("recycled message: {:?}", read_msg);
        //     queue.archive(queue_name, &read_msg.msg_id).await?;
        //     continue;
        // }

        // Based on message_type in message, create, update, delete PostgresCluster
        match read_msg.message.event_type {
            Event::Create | Event::Update => {
                info!("Doing nothing for now");
                create_namespace(client.clone(), &read_msg.message.dbname)
                    .await
                    .expect("error creating namespace");

                // create IngressRouteTCP
                create_ing_route_tcp(client.clone(), &read_msg.message.dbname)
                    .await
                    .expect("error creating IngressRouteTCP");

                // create /metrics ingress
                create_metrics_ingress(client.clone(), &read_msg.message.dbname)
                    .await
                    .expect("error creating ingress for /metrics");

                // generate PostgresCluster spec based on values in body
                let spec = generate_spec(&read_msg.message.dbname, &read_msg.message.spec).await;

                // create or update PostgresCluster
                create_or_update(client.clone(), &read_msg.message.dbname, spec)
                    .await
                    .expect("error creating or updating PostgresCluster");
                // get connection string values from secret
                let connection_string = get_pg_conn(client.clone(), &read_msg.message.dbname)
                    .await
                    .expect("error getting secret");

                let report_event = match read_msg.message.event_type {
                    Event::Create => Event::Created,
                    Event::Update => Event::Updated,
                    _ => unreachable!(),
                };
                let msg = types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    event_id: read_msg.message.event_id,
                    event_type: report_event,
                    spec: Some(read_msg.message.spec.clone()),
                    connection: Some(connection_string),
                };
                let msg_id = queue.send(&data_plane_events_queue, &msg).await?;
                info!("msg_id: {:?}", msg_id);
            }
            Event::Delete => {
                info!("Doing nothing for now");
                // delete PostgresCluster
                delete(
                    client.clone(),
                    &read_msg.message.dbname,
                    &read_msg.message.dbname,
                )
                .await
                .expect("error deleting PostgresCluster");

                // delete namespace
                delete_namespace(client.clone(), &read_msg.message.dbname)
                    .await
                    .expect("error deleting namespace");

                // report state
                let msg = types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    event_id: read_msg.message.event_id,
                    event_type: Event::Deleted,
                    spec: None,
                    connection: None,
                };
                let msg_id = queue.send(&data_plane_events_queue, &msg).await?;
                info!("msg_id: {:?}", msg_id);
            }
            _ => {
                warn!("action was not in expected format");
                continue;
            }
        }

        // TODO (ianstanton) This is here as an example for now. We want to use
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
