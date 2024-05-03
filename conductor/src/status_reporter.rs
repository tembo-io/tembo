use conductor::errors::ConductorError;
use controller::apis::coredb_types::CoreDB;
use futures::TryStreamExt;
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Api, Client};
use log::{error, info, warn};
use pgmq::PGMQueueExt;
use std::env;

use conductor::monitoring::CustomMetrics;
use conductor::types::Event;
use conductor::{get_data_plane_id_from_coredb, get_org_inst_id, get_pg_conn, types};

pub async fn run_status_reporter(
    _metrics: CustomMetrics,
) -> Result<(), Box<dyn std::error::Error>> {
    // Move to config
    let pg_conn_url =
        env::var("POSTGRES_QUEUE_CONNECTION").expect("POSTGRES_QUEUE_CONNECTION must be set");
    // Connect to pgmq
    let queue = PGMQueueExt::new(pg_conn_url.clone(), 1).await?;

    // Get a kubernetes watcher on all changes in coredb resources
    let client = Client::try_default().await?;
    let coredb_api: Api<CoreDB> = Api::all(client.clone());

    watcher(coredb_api, watcher::Config::default())
        .applied_objects()
        .try_for_each(move |coredb| {
            let client = client.clone();
            let queue = queue.clone();
            async move {
                info!(
                    "Detected change in coredb: {}",
                    coredb
                        .metadata
                        .name
                        .as_ref()
                        .expect("CoreDB should always have a name")
                );
                match send_status_update(client, &queue, coredb).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Error sending status update: {}", e);
                    }
                };
                Ok(())
            }
        })
        .await?;
    Ok(())
}

// Used for sending ad-hoc status updates to the control plane.
// This can be triggered when a change in a CoreDB's status is detected.
async fn send_status_update(
    client: Client,
    response_queue: &PGMQueueExt,
    coredb: CoreDB,
) -> Result<(), ConductorError> {
    let coredb_name = &coredb
        .metadata
        .name
        .as_ref()
        .expect("CoreDB should always have a name");
    let namespace = coredb
        .metadata
        .namespace
        .as_ref()
        .expect("CoreDB should always have a namespace");
    // Could be nice to move these env reads into a config struct
    let data_plane_basedomain = match env::var("DATA_PLANE_BASEDOMAIN") {
        Ok(domain) => domain,
        Err(_) => {
            error!("DATA_PLANE_BASEDOMAIN is not set, skipping status update");
            return Ok(());
        }
    };
    let data_plane_events_queue = match env::var("DATA_PLANE_EVENTS_QUEUE") {
        Ok(data_plane_events_queue) => data_plane_events_queue,
        Err(_) => {
            error!("DATA_PLANE_EVENTS_QUEUE is not set, skipping status update");
            return Ok(());
        }
    };
    let org_inst = match get_org_inst_id(&coredb) {
        Ok(org_inst) => org_inst,
        Err(_) => {
            warn!("Could not get org_id and inst_id from CoreDB {}, needs to be updated with annotations, which will happen on the next update from control plane, skipping", coredb_name);
            return Ok(());
        }
    };

    let data_plane_id = match get_data_plane_id_from_coredb(&coredb) {
        Ok(dp_id) => dp_id,
        Err(_) => {
            warn!("Could not get data_plane_id from CoreDB {}, needs to be updated with annotations, which will happen on the next update from control plane, skipping", coredb_name);
            return Ok(());
        }
    };

    let conn_info = match get_pg_conn(client, namespace, &data_plane_basedomain, &coredb.spec).await
    {
        Ok(conn_info) => conn_info,
        Err(_) => {
            info!("Could not get connection info for CoreDB {}, skipping status update. This can be normal for a few seconds when the resource is initially created, and when the instance is being deleted.", coredb_name);
            return Ok(());
        }
    };
    let response = types::StateToControlPlane {
        data_plane_id,
        org_id: org_inst.org_id.clone(),
        inst_id: org_inst.inst_id.clone(),
        event_type: Event::Updated,
        spec: Some(coredb.spec.clone()),
        status: coredb.status.clone(),
        connection: Some(conn_info),
    };
    let msg_id = response_queue
        .send(&data_plane_events_queue, &response)
        .await?;
    info!(
        "{}.{}: Sent ad hoc update to control plane, message_id: {}",
        org_inst.org_id, org_inst.inst_id, msg_id
    );
    Ok(())
}
