use conductor::errors::ConductorError;
use conductor::{
    create_cloudformation, create_namespace, create_networkpolicy, create_or_update, delete,
    delete_cloudformation, delete_namespace, extensions::extension_plan, generate_rand_schedule,
    generate_spec, get_coredb_status, get_pg_conn, lookup_role_arn, restart_statefulset, types,
};
use controller::apis::coredb_types::{Backup, CoreDBSpec, ServiceAccountTemplate};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Client;
use log::{debug, error, info, warn};
use pgmq::{Message, PGMQueueExt};
use std::env;
use std::{thread, time};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;
use types::{CRUDevent, Event};
#[tokio::main]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Read connection info from environment variable
    let pg_conn_url =
        env::var("POSTGRES_QUEUE_CONNECTION").expect("POSTGRES_QUEUE_CONNECTION must be set");
    let control_plane_events_queue =
        env::var("CONTROL_PLANE_EVENTS_QUEUE").expect("CONTROL_PLANE_EVENTS_QUEUE must be set");
    let data_plane_events_queue =
        env::var("DATA_PLANE_EVENTS_QUEUE").expect("DATA_PLANE_EVENTS_QUEUE must be set");
    let data_plane_basedomain =
        env::var("DATA_PLANE_BASEDOMAIN").expect("DATA_PLANE_BASEDOMAIN must be set");
    let backup_archive_bucket =
        env::var("BACKUP_ARCHIVE_BUCKET").expect("BACKUP_ARCHIVE_BUCKET must be set");
    let cf_template_bucket =
        env::var("CF_TEMPLATE_BUCKET").expect("CF_TEMPLATE_BUCKET must be set");
    let max_read_ct: i32 = env::var("MAX_READ_CT")
        .unwrap_or_else(|_| "100".to_owned())
        .parse()
        .expect("error parsing MAX_READ_CT");

    // Connect to pgmq
    let queue = PGMQueueExt::new(pg_conn_url, 5).await?;
    queue.init().await?;

    // Create queues if they do not exist
    queue.create(&control_plane_events_queue).await?;
    queue.create(&data_plane_events_queue).await?;

    // Infer the runtime environment and try to create a Kubernetes Client
    let client = Client::try_default().await?;

    // Amount of time to wait after requeueing a message for an expected failure,
    // where we will want to check often until it's ready.
    const REQUEUE_VT_SEC_SHORT: i32 = 5;

    // Amount of time to wait after requeueing a message for an unexpected failure
    // that we would want to try again after awhile.
    const REQUEUE_VT_SEC_LONG: i32 = 300;

    loop {
        // Read from queue (check for new message)
        // messages that dont fit a CRUDevent will error
        // set visibility timeout to 90 seconds
        let read_msg = queue
            .read::<CRUDevent>(&control_plane_events_queue, 90_i32)
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

        // note: messages are recycled on purpose
        // but absurdly high read_ct means its probably never going to get processed
        if read_msg.read_ct >= max_read_ct {
            error!(
                "archived message with read_count >= `{}`: {:?}",
                max_read_ct, read_msg
            );
            queue
                .archive(&control_plane_events_queue, read_msg.msg_id)
                .await?;
            // this is what we'll send back to control-plane
            let error_event = types::StateToControlPlane {
                data_plane_id: read_msg.message.data_plane_id,
                event_id: read_msg.message.event_id,
                event_type: Event::Error,
                spec: None,
                connection: None,
            };
            let msg_id = queue.send(&data_plane_events_queue, &error_event).await?;
            error!("sent error event to control-plane, msg_id: {:?}", msg_id);
            continue;
        }

        let namespace = format!(
            "org-{}-inst-{}",
            read_msg.message.organization_name, read_msg.message.dbname
        );

        // Based on message_type in message, create, update, delete CoreDB
        let event_msg: types::StateToControlPlane = match read_msg.message.event_type {
            // every event is for a single namespace
            Event::Create | Event::Update => {
                // (todo: nhudson) in thr future move this to be more specific
                // to the event that we are taking action on.  For now just create
                // the stack without checking.

                if read_msg.message.spec.is_none() {
                    error!(
                        "spec is required on create and update events, archiving message {}",
                        read_msg.msg_id
                    );
                    let _archived = queue
                        .archive(&control_plane_events_queue, read_msg.msg_id)
                        .await
                        .expect("error archiving message from queue");
                    continue;
                }
                // spec.expect() should be safe here - since above we continue in loop when it is None
                let msg_spec = read_msg.message.spec.clone().expect("message spec");

                create_cloudformation(
                    String::from("us-east-1"),
                    backup_archive_bucket.clone(),
                    &read_msg.message.organization_name,
                    &read_msg.message.dbname,
                    &cf_template_bucket,
                )
                .await?;

                // Lookup the CloudFormation stack's role ARN
                let role_arn = match lookup_role_arn(
                    String::from("us-east-1"),
                    &read_msg.message.organization_name,
                    &read_msg.message.dbname,
                )
                .await
                {
                    Ok(arn) => arn,
                    Err(err) => match err {
                        ConductorError::NoOutputsFound => {
                            info!("CloudFormation stack outputs not ready, requeuing with short duration. message id {}", read_msg.msg_id);
                            // Requeue the message for a short duration
                            let _ = queue
                                .set_vt::<CRUDevent>(
                                    &control_plane_events_queue,
                                    read_msg.msg_id,
                                    REQUEUE_VT_SEC_SHORT,
                                )
                                .await?;
                            continue;
                        }
                        _ => {
                            error!(
                                "Failed to get stack outputs for message id {}: {}",
                                read_msg.msg_id, err
                            );
                            let _ = queue
                                .set_vt::<CRUDevent>(
                                    &control_plane_events_queue,
                                    read_msg.msg_id,
                                    REQUEUE_VT_SEC_LONG,
                                )
                                .await?;
                            continue;
                        }
                    },
                };

                // Format ServiceAccountTemplate spec in CoreDBSpec
                use std::collections::BTreeMap;
                let mut annotations: BTreeMap<String, String> = BTreeMap::new();
                annotations.insert("eks.amazonaws.com/role-arn".to_string(), role_arn.clone());
                let service_account_template = ServiceAccountTemplate {
                    metadata: Some(ObjectMeta {
                        annotations: Some(annotations),
                        ..ObjectMeta::default()
                    }),
                };

                // Format Backup spec in CoreDBSpec
                let backup = Backup {
                    destinationPath: Some(format!(
                        "s3://{}/coredb/{}/org-{}-inst-{}",
                        backup_archive_bucket,
                        &read_msg.message.organization_name,
                        &read_msg.message.organization_name,
                        &read_msg.message.dbname
                    )),
                    encryption: Some(String::from("AES256")),
                    retentionPolicy: Some(String::from("30")),
                    schedule: Some(generate_rand_schedule().await),
                };

                // Merge backup and service_account_template into spec
                let coredb_spec = CoreDBSpec {
                    serviceAccountTemplate: service_account_template,
                    backup,
                    ..msg_spec.clone()
                };
                // create Namespace
                create_namespace(client.clone(), &namespace).await?;

                // create NetworkPolicy to allow internet access only
                create_networkpolicy(client.clone(), &namespace).await?;

                // generate CoreDB spec based on values in body
                let spec = generate_spec(&namespace, &coredb_spec).await;

                // create or update CoreDB
                create_or_update(client.clone(), &namespace, spec).await?;

                // get connection string values from secret

                let conn_info = match get_pg_conn(
                    client.clone(),
                    &namespace,
                    &data_plane_basedomain,
                )
                .await
                {
                    Ok(conn_info) => conn_info,
                    Err(err) => {
                        match err {
                            ConductorError::PostgresConnectionInfoNotFound => {
                                info!("Secret not ready, requeuing with short duration. message id {}", read_msg.msg_id);
                                // Requeue the message for a short duration
                                let _ = queue
                                    .set_vt::<CRUDevent>(
                                        &control_plane_events_queue,
                                        read_msg.msg_id,
                                        REQUEUE_VT_SEC_SHORT,
                                    )
                                    .await?;
                                continue;
                            }
                            _ => {
                                error!(
                                    "Error getting Postgres connection information from secret for message id {}: {}",
                                    read_msg.msg_id, err
                                );
                                let _ = queue
                                    .set_vt::<CRUDevent>(
                                        &control_plane_events_queue,
                                        read_msg.msg_id,
                                        REQUEUE_VT_SEC_LONG,
                                    )
                                    .await?;
                                continue;
                            }
                        }
                    }
                };

                let result = get_coredb_status(client.clone(), &namespace).await;

                // determine if we should requeue the message
                let requeue: bool = match &result {
                    Ok(current_spec) => {
                        // if the coredb is still updating the extensions, requeue this task and try again in a few seconds
                        let status = current_spec.clone().status.expect("no status present");
                        let updating_extension = status.extensionsUpdating;

                        // requeue when extensions are "out of sync"
                        // this happens when:
                        // 1. no extensions reported on the crd status
                        // 2. number of desired extensions != actual extensions
                        // 3. there is a difference in hashes between desired and actual extensions
                        let extensions_out_of_sync: bool = match status.extensions {
                            Some(actual_extensions) => {
                                // requeue if there are less extensions than desired
                                // likely means that the extensions are still being updated
                                // or there is an issue changing an extension
                                let desired_extensions = msg_spec.extensions;
                                // if no extensions in request, then exit
                                if desired_extensions.is_empty() {
                                    info!("No extensions in request");
                                    false
                                } else {
                                    let (changed, to_install) =
                                        extension_plan(&desired_extensions, &actual_extensions);
                                    // requeue if extensions need to be installed or updated
                                    if !changed.is_empty() || !to_install.is_empty() {
                                        warn!(
                                            "changed: {:?}, to_install: {:?}",
                                            changed, to_install
                                        );
                                        true
                                    } else {
                                        false
                                    }
                                }
                            }
                            None => true,
                        };
                        if updating_extension || extensions_out_of_sync {
                            warn!(
                                "extensions updating: {}, extensions_out_of_sync: {}",
                                updating_extension, extensions_out_of_sync
                            );
                            true
                        } else {
                            false
                        }
                    }
                    Err(err) => {
                        error!(
                            "error getting CoreDB status in {}: {:?}",
                            namespace.clone(),
                            err
                        );
                        true
                    }
                };

                if requeue {
                    // requeue then continue loop from beginning
                    let _ = queue
                        .set_vt::<CRUDevent>(
                            &control_plane_events_queue,
                            read_msg.msg_id,
                            REQUEUE_VT_SEC_SHORT,
                        )
                        .await?;
                    continue;
                }

                let mut current_spec = result?;

                let spec_js = serde_json::to_string(&current_spec.spec).unwrap();
                debug!("dbname: {}, current_spec: {:?}", &namespace, spec_js);

                // get actual extensions from crd status
                // UPDATE SPEC OBJECT WITH ACTUAL EXTENSIONS
                current_spec.spec.extensions = current_spec
                    .status
                    .and_then(|o| o.extensions)
                    .unwrap_or_default();

                let report_event = match read_msg.message.event_type {
                    Event::Create => Event::Created,
                    Event::Update => Event::Updated,
                    _ => unreachable!(),
                };
                types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    event_id: read_msg.message.event_id,
                    event_type: report_event,
                    spec: Some(current_spec.spec),
                    connection: Some(conn_info),
                }
            }
            Event::Delete => {
                // delete CoreDB
                delete(client.clone(), &namespace, &namespace).await?;

                // delete namespace
                delete_namespace(client.clone(), &namespace).await?;

                delete_cloudformation(
                    String::from("us-east-1"),
                    &read_msg.message.organization_name,
                    &read_msg.message.dbname,
                )
                .await?;

                // report state
                types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    event_id: read_msg.message.event_id,
                    event_type: Event::Deleted,
                    spec: None,
                    connection: None,
                }
            }
            Event::Restart => {
                // TODO: refactor to be more DRY
                // Restart and Update events share a lot of the same code.
                // move some operations after the Event match
                info!("handling instance restart");
                match restart_statefulset(client.clone(), &namespace, &namespace).await {
                    Ok(_) => {}
                    Err(err) => {
                        error!("error restarting statefulset: {:?}", err);
                        continue;
                    }
                }
                let retry_strategy = FixedInterval::from_millis(5000).take(20);
                let result = Retry::spawn(retry_strategy.clone(), || {
                    get_coredb_status(client.clone(), &namespace)
                })
                .await;
                if result.is_err() {
                    error!(
                        "error getting CoreDB status in {}: {:?}",
                        namespace.clone(),
                        result
                    );
                    continue;
                }
                let mut current_spec = result?;
                let spec_js = serde_json::to_string(&current_spec.spec).unwrap();
                debug!("dbname: {}, current_spec: {:?}", &namespace, spec_js);

                // get actual extensions from crd status
                // UPDATE SPEC OBJECT WITH ACTUAL EXTENSIONS
                current_spec.spec.extensions = current_spec
                    .status
                    .and_then(|o| o.extensions)
                    .unwrap_or_default();

                let conn_info =
                    get_pg_conn(client.clone(), &namespace, &data_plane_basedomain).await;

                types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    event_id: read_msg.message.event_id,
                    event_type: Event::Restarted,
                    spec: Some(current_spec.spec),
                    connection: conn_info.ok(),
                }
            }
            _ => {
                warn!("Unhandled event_type: {:?}", read_msg.message.event_type);
                continue;
            }
        };
        let msg_id = queue.send(&data_plane_events_queue, &event_msg).await?;
        debug!("sent msg_id: {:?}", msg_id);

        // archive message from queue
        let archived = queue
            .archive(&control_plane_events_queue, read_msg.msg_id)
            .await
            .expect("error archiving message from queue");
        // TODO(ianstanton) Improve logging everywhere
        info!("archived: {:?}", archived);
    }
}

fn main() {
    env_logger::init();
    info!("starting");
    loop {
        match run() {
            Ok(_) => {}
            Err(err) => {
                error!("error: {:?}", err);
            }
        }
    }
}
