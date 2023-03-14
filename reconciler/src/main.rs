use kube::{Client, ResourceExt};
use log::{debug, error, info, warn};
use pgmq::{Message, PGMQueue};
use reconciler::{
    create_ing_route_tcp, create_metrics_ingress, create_namespace, create_or_update, delete,
    delete_namespace, generate_spec, get_all, get_coredb_status, get_pg_conn, types,
};
use std::env;
use std::{thread, time};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;
use types::{CRUDevent, Event};

#[tokio::main]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Read connection info from environment variable
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
        // set visibility timeout to 90 seconds
        let read_msg = queue
            .read::<CRUDevent>(&control_plane_events_queue, Some(&90_i32))
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

        // TODO: recycled messages should get archived, logged, alerted
        // this auto-archive of bad messages should only get implemented after
        // control-plane has a scheduled reconciler process implemented
        // if read_msg.read_ct >= 2 {
        //     warn!("recycled message: {:?}", read_msg);
        //     queue.archive(queue_name, &read_msg.msg_id).await?;
        //     continue;
        // }

        // Based on message_type in message, create, update, delete CoreDB
        match read_msg.message.event_type {
            // every event is for a single namespace
            Event::Create | Event::Update => {
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

                // generate CoreDB spec based on values in body
                let spec = generate_spec(&read_msg.message.dbname, &read_msg.message.spec).await;

                let spec_js = serde_json::to_string(&spec).unwrap();
                debug!("spec: {}", spec_js);
                // create or update CoreDB
                create_or_update(client.clone(), &read_msg.message.dbname, spec)
                    .await
                    .expect("error creating or updating CoreDB");
                // get connection string values from secret
                let connection_string = get_pg_conn(client.clone(), &read_msg.message.dbname)
                    .await
                    .expect("error getting secret");

                // read spec.status from CoreDB
                // this should wait until it is able to receive an actual update from the cluster
                // retrying actions with kube
                // limit to 60 seconds - 20 retries, 5 seconds between retries
                // TODO: need a better way to handle this
                let retry_strategy = FixedInterval::from_millis(5000).take(20);
                let result = Retry::spawn(retry_strategy.clone(), || {
                    get_coredb_status(client.clone(), &read_msg.message.dbname)
                })
                .await;
                if result.is_err() {
                    error!("error getting CoreDB status: {:?}", result);
                    continue;
                }
                let mut current_spec = result?;
                let spec_js = serde_json::to_string(&current_spec.spec).unwrap();
                debug!(
                    "dbname: {}, current_spec: {:?}",
                    &read_msg.message.dbname, spec_js
                );

                // get actual extensions from crd status
                let actual_extension = match current_spec.status {
                    Some(status) => status.extensions,
                    None => {
                        warn!("No extensions in: {:?}", &read_msg.message.dbname);
                        None
                    }
                };
                // UPDATE SPEC OBJECT WITH ACTUAL EXTENSIONS
                current_spec.spec.extensions = actual_extension;

                let report_event = match read_msg.message.event_type {
                    Event::Create => Event::Created,
                    Event::Update => Event::Updated,
                    _ => unreachable!(),
                };
                let msg = types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    event_id: read_msg.message.event_id,
                    event_type: report_event,
                    spec: Some(current_spec.spec),
                    connection: Some(connection_string),
                };
                let msg_id = queue.send(&data_plane_events_queue, &msg).await?;
                info!("sent msg_id: {:?}", msg_id);
            }
            Event::Delete => {
                // delete CoreDB
                delete(
                    client.clone(),
                    &read_msg.message.dbname,
                    &read_msg.message.dbname,
                )
                .await
                .expect("error deleting CoreDB");

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
                info!("sent msg_id: {:?}", msg_id);
            }
            _ => {
                warn!("action was not in expected format");
                continue;
            }
        }

        // TODO (ianstanton) This is here as an example for now. We want to use
        //  this to ensure a CoreDB exists before we attempt to delete it.
        // Get all existing CoreDB
        let vec = get_all(client.clone(), "default");
        for pg in vec.await.iter() {
            info!("found CoreDB {}", pg.name_any());
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
