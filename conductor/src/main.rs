use actix_web::{web, App, HttpServer};
use actix_web_opentelemetry::{PrometheusMetricsHandler, RequestTracing};
use conductor::errors::ConductorError;
use conductor::extensions::extensions_still_processing;
use conductor::monitoring::CustomMetrics;
use conductor::{
    create_cloudformation, create_namespace, create_networkpolicy, create_or_update, delete,
    delete_cloudformation, delete_namespace, generate_rand_schedule, generate_spec,
    get_coredb_error_without_status, get_one, get_pg_conn, lookup_role_arn, parse_event_id,
    restart_cnpg, restart_statefulset, types,
};
use controller::apis::coredb_types::{Backup, CoreDBSpec, S3Credentials, ServiceAccountTemplate};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Client;
use log::{debug, error, info, warn};
use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::{controllers, processors, selectors};
use opentelemetry::{global, KeyValue};
use pgmq::{Message, PGMQueueExt};
use std::env;
use std::sync::{Arc, Mutex};
use std::{thread, time};

use crate::status_reporter::run_status_reporter;
use conductor::routes::health::background_threads_running;
use types::{CRUDevent, Event};

mod status_reporter;

async fn run(metrics: CustomMetrics) -> Result<(), Box<dyn std::error::Error>> {
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
                debug!("no messages in queue");
                thread::sleep(time::Duration::from_secs(1));
                continue;
            }
        };

        metrics
            .conductor_total
            .add(&opentelemetry::Context::current(), 1, &[]);

        // note: messages are recycled on purpose
        // but absurdly high read_ct means its probably never going to get processed
        if read_msg.read_ct >= max_read_ct {
            error!(
                "{}: archived message with read_count >= `{}`: {:?}",
                read_msg.msg_id, max_read_ct, read_msg
            );
            queue
                .archive(&control_plane_events_queue, read_msg.msg_id)
                .await?;
            metrics
                .conductor_errors
                .add(&opentelemetry::Context::current(), 1, &[]);
            // this is what we'll send back to control-plane
            let error_event = types::StateToControlPlane {
                data_plane_id: read_msg.message.data_plane_id,
                event_id: read_msg.message.event_id,
                event_type: Event::Error,
                spec: None,
                status: None,
                connection: None,
            };
            let msg_id = queue.send(&data_plane_events_queue, &error_event).await?;
            error!(
                "{}: sent error event to control-plane: {}",
                read_msg.msg_id, msg_id
            );
            continue;
        }

        let namespace = format!(
            "org-{}-inst-{}",
            read_msg.message.organization_name, read_msg.message.dbname
        );
        info!("{}: Using namespace {}", read_msg.msg_id, &namespace);

        // Based on message_type in message, create, update, delete CoreDB
        let event_msg: types::StateToControlPlane = match read_msg.message.event_type {
            // every event is for a single namespace
            Event::Create | Event::Update => {
                info!("{}: Got create or update event", read_msg.msg_id);

                // (todo: nhudson) in thr future move this to be more specific
                // to the event that we are taking action on.  For now just create
                // the stack without checking.

                if read_msg.message.spec.is_none() {
                    error!(
                        "{}: spec is required on create and update events, archiving message",
                        read_msg.msg_id
                    );
                    let _archived = queue
                        .archive(&control_plane_events_queue, read_msg.msg_id)
                        .await
                        .expect("error archiving message from queue");
                    metrics
                        .conductor_errors
                        .add(&opentelemetry::Context::current(), 1, &[]);
                    continue;
                }
                // spec.expect() should be safe here - since above we continue in loop when it is None
                let msg_spec = read_msg.message.spec.clone().expect("message spec");

                info!("{}: Creating cloudformation template", read_msg.msg_id);
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
                    Ok(arn) => {
                        info!(
                            "{}: CloudFormation stack outputs ready, got outputs.",
                            read_msg.msg_id
                        );
                        arn
                    }
                    Err(err) => match err {
                        ConductorError::NoOutputsFound => {
                            info!("{}: CloudFormation stack outputs not ready, requeuing with short duration.", read_msg.msg_id);
                            // Requeue the message for a short duration
                            let _ = queue
                                .set_vt::<CRUDevent>(
                                    &control_plane_events_queue,
                                    read_msg.msg_id,
                                    REQUEUE_VT_SEC_SHORT,
                                )
                                .await?;
                            metrics.conductor_requeues.add(
                                &opentelemetry::Context::current(),
                                1,
                                &[KeyValue::new("queue_duration", "short")],
                            );
                            continue;
                        }
                        _ => {
                            error!(
                                "{}: Failed to get stack outputs with error: {}",
                                read_msg.msg_id, err
                            );
                            let _ = queue
                                .set_vt::<CRUDevent>(
                                    &control_plane_events_queue,
                                    read_msg.msg_id,
                                    REQUEUE_VT_SEC_LONG,
                                )
                                .await?;
                            metrics.conductor_requeues.add(
                                &opentelemetry::Context::current(),
                                1,
                                &[KeyValue::new("queue_duration", "long")],
                            );
                            continue;
                        }
                    },
                };

                info!("{}: Adding backup configuration to spec", read_msg.msg_id);
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
                    s3_credentials: Some(S3Credentials {
                        inherit_from_iam_role: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                };

                // Merge backup and service_account_template into spec
                let coredb_spec = CoreDBSpec {
                    serviceAccountTemplate: service_account_template,
                    backup,
                    ..msg_spec.clone()
                };

                info!("{}: Creating namespace", read_msg.msg_id);
                // create Namespace
                create_namespace(client.clone(), &namespace).await?;

                info!("{}: Creating network policy", read_msg.msg_id);
                // create NetworkPolicy to allow internet access only
                create_networkpolicy(client.clone(), &namespace).await?;

                info!("{}: Generating spec", read_msg.msg_id);
                // generate CoreDB spec based on values in body
                let (workspace_id, org_id, entity_name, instance_id) =
                    parse_event_id(read_msg.message.event_id.as_str())?;
                let spec = generate_spec(
                    &workspace_id,
                    &org_id,
                    &entity_name,
                    &instance_id,
                    &read_msg.message.data_plane_id,
                    &namespace,
                    &coredb_spec,
                )
                .await;

                info!("{}: Creating or updating spec", read_msg.msg_id);
                // create or update CoreDB
                create_or_update(client.clone(), &namespace, spec).await?;

                // get connection string values from secret

                info!("{}: Getting connection info", read_msg.msg_id);
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
                                info!(
                                    "{}: Secret not ready, requeuing with short duration.",
                                    read_msg.msg_id
                                );
                                // Requeue the message for a short duration
                                let _ = queue
                                    .set_vt::<CRUDevent>(
                                        &control_plane_events_queue,
                                        read_msg.msg_id,
                                        REQUEUE_VT_SEC_SHORT,
                                    )
                                    .await?;
                                metrics.conductor_requeues.add(
                                    &opentelemetry::Context::current(),
                                    1,
                                    &[KeyValue::new("queue_duration", "short")],
                                );
                                continue;
                            }
                            _ => {
                                error!(
                                    "{}: Error getting Postgres connection information from secret: {}",
                                    read_msg.msg_id, err
                                );
                                let _ = queue
                                    .set_vt::<CRUDevent>(
                                        &control_plane_events_queue,
                                        read_msg.msg_id,
                                        REQUEUE_VT_SEC_LONG,
                                    )
                                    .await?;
                                metrics.conductor_errors.add(
                                    &opentelemetry::Context::current(),
                                    1,
                                    &[],
                                );
                                continue;
                            }
                        }
                    }
                };

                info!("{}: Getting status", read_msg.msg_id);

                let result = get_one(client.clone(), &namespace).await;

                let extension_still_processing = match &result {
                    Ok(coredb) => extensions_still_processing(coredb),
                    Err(_) => true,
                };

                if extension_still_processing && read_msg.message.event_type == Event::Create {
                    let _ = queue
                        .set_vt::<CRUDevent>(
                            &control_plane_events_queue,
                            read_msg.msg_id,
                            REQUEUE_VT_SEC_SHORT,
                        )
                        .await?;
                    metrics.conductor_requeues.add(
                        &opentelemetry::Context::current(),
                        1,
                        &[KeyValue::new("queue_duration", "short")],
                    );
                    continue;
                }

                let current_spec = result?;

                let spec_js = serde_json::to_string(&current_spec.spec).unwrap();
                debug!("dbname: {}, current_spec: {:?}", &namespace, spec_js);

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
                    status: current_spec.status,
                    connection: Some(conn_info),
                }
            }
            Event::Delete => {
                // delete CoreDB
                info!("{}: Deleting instance {}", read_msg.msg_id, &namespace);
                delete(client.clone(), &namespace, &namespace).await?;

                // delete namespace
                info!("{}: Deleting namespace {}", read_msg.msg_id, &namespace);
                delete_namespace(client.clone(), &namespace).await?;

                info!("{}: Deleting cloudformation stack", read_msg.msg_id);
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
                    status: None,
                    connection: None,
                }
            }
            Event::Restart => {
                // TODO: refactor to be more DRY
                // Restart and Update events share a lot of the same code.
                // move some operations after the Event match
                info!("{}: handling instance restart", read_msg.msg_id);
                match restart_statefulset(client.clone(), &namespace, &namespace).await {
                    Ok(_) => {}
                    Err(err) => {
                        error!("error restarting statefulset: {:?}", err);
                    }
                }
                match restart_cnpg(client.clone(), &namespace, &namespace).await {
                    Ok(_) => {}
                    Err(err) => {
                        error!("error restarting cnpg: {:?}", err);
                    }
                }

                let result = get_coredb_error_without_status(client.clone(), &namespace).await;

                let current_resource = match result {
                    Ok(coredb) => {
                        // Safety: we know status exists due to get_coredb_error_without_status
                        let status = coredb.status.as_ref().unwrap();
                        if !status.running {
                            // Instance is still rebooting, recheck this message later
                            let _ = queue
                                .set_vt::<CRUDevent>(
                                    &control_plane_events_queue,
                                    read_msg.msg_id,
                                    REQUEUE_VT_SEC_SHORT,
                                )
                                .await?;
                            metrics.conductor_requeues.add(
                                &opentelemetry::Context::current(),
                                1,
                                &[KeyValue::new("queue_duration", "short")],
                            );
                        }

                        let as_json = serde_json::to_string(&coredb);
                        debug!("dbname: {}, current: {:?}", &namespace, as_json);

                        coredb
                    }
                    Err(_) => {
                        error!("error getting CoreDB status in {}: {:?}", namespace, result);
                        metrics
                            .conductor_errors
                            .add(&opentelemetry::Context::current(), 1, &[]);

                        continue;
                    }
                };

                let conn_info =
                    get_pg_conn(client.clone(), &namespace, &data_plane_basedomain).await;

                types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    event_id: read_msg.message.event_id,
                    event_type: Event::Restarted,
                    spec: Some(current_resource.spec),
                    status: current_resource.status,
                    connection: conn_info.ok(),
                }
            }
            _ => {
                warn!("Unhandled event_type: {:?}", read_msg.message.event_type);
                metrics
                    .conductor_errors
                    .add(&opentelemetry::Context::current(), 1, &[]);
                continue;
            }
        };

        let msg_id = queue.send(&data_plane_events_queue, &event_msg).await?;
        info!(
            "{}: responded to control plane with message {}",
            read_msg.msg_id, msg_id
        );

        // archive message from queue
        let archived = queue
            .archive(&control_plane_events_queue, read_msg.msg_id)
            .await
            .expect("error archiving message from queue");

        metrics
            .conductor_completed
            .add(&opentelemetry::Context::current(), 1, &[]);

        info!("{}: archived: {:?}", read_msg.msg_id, archived);
    }
}

// https://github.com/rust-lang/rust-clippy/issues/6446
// False positive because lock is dropped before await
#[allow(clippy::await_holding_lock)]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let controller = controllers::basic(
        processors::factory(
            selectors::simple::histogram([1.0, 2.0, 5.0, 10.0, 20.0, 50.0]),
            aggregation::cumulative_temporality_selector(),
        )
        .with_memory(true),
    )
    .build();

    let exporter = opentelemetry_prometheus::exporter(controller).init();
    let meter = global::meter("actix_web");
    let custom_metrics = CustomMetrics::new(&meter);

    let background_threads: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let mut background_threads_locked = background_threads
        .lock()
        .expect("Failed to remember our background threads");

    info!("Starting conductor");
    background_threads_locked.push(tokio::spawn({
        let custom_metrics_copy = custom_metrics.clone();
        async move {
            loop {
                match run(custom_metrics_copy.clone()).await {
                    Ok(_) => {}
                    Err(err) => {
                        custom_metrics_copy.clone().conductor_errors.add(
                            &opentelemetry::Context::current(),
                            1,
                            &[],
                        );
                        error!("error in conductor: {:?}", err);
                    }
                }
                warn!("conductor exited, sleeping for 1 second");
                thread::sleep(time::Duration::from_secs(1));
            }
        }
    }));

    info!("Starting status reporter");
    background_threads_locked.push(tokio::spawn({
        let custom_metrics_copy = custom_metrics.clone();
        async move {
            loop {
                match run_status_reporter(custom_metrics_copy.clone()).await {
                    Ok(_) => {}
                    Err(err) => {
                        custom_metrics_copy.clone().conductor_errors.add(
                            &opentelemetry::Context::current(),
                            1,
                            &[],
                        );
                        error!("error in conductor: {:?}", err);
                    }
                }
                warn!("conductor exited, sleeping for 1 second");
                thread::sleep(time::Duration::from_secs(1));
            }
        }
    }));

    std::mem::drop(background_threads_locked);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(custom_metrics.clone()))
            .app_data(web::Data::new(background_threads.clone()))
            .wrap(RequestTracing::new())
            .route(
                "/metrics",
                web::get().to(PrometheusMetricsHandler::new(exporter.clone())),
            )
            .service(web::scope("/health").service(background_threads_running))
    })
    .workers(1)
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
