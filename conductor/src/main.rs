use actix_web::{web, App, HttpServer};
use actix_web_opentelemetry::{PrometheusMetricsHandler, RequestTracing};
use conductor::errors::ConductorError;
use conductor::monitoring::CustomMetrics;
use conductor::{
    cloud::CloudProvider, create_azure_storage_workload_identity_binding, create_cloudformation,
    create_gcp_storage_workload_identity_binding, create_namespace, create_or_update, delete,
    delete_azure_storage_workload_identity_binding, delete_cloudformation,
    delete_gcp_storage_workload_identity_binding, delete_namespace, generate_cron_expression,
    generate_spec, get_coredb_error_without_status, get_one, get_pg_conn, lookup_role_arn,
    restart_coredb, types,
};

use crate::metrics_reporter::run_metrics_reporter;
use crate::status_reporter::run_status_reporter;
use conductor::routes::health::background_threads_running;
use controller::apis::coredb_types::{
    AzureCredentials, Backup, CoreDBSpec, GoogleCredentials, S3Credentials,
    S3CredentialsAccessKeyId, S3CredentialsSecretAccessKey, ServiceAccountTemplate, VolumeSnapshot,
};
use controller::apis::postgres_parameters::{ConfigValue, PgConfig};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Client;
use log::{debug, error, info, warn};
use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::{controllers, processors, selectors};
use opentelemetry::{global, KeyValue};
use pgmq::{Message, PGMQueueExt};
use sqlx::error::Error;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::{Arc, Mutex};
use std::time;
use types::{CRUDevent, Event};

mod metrics_reporter;
mod status_reporter;

// Amount of time to wait after requeueing a message for an expected failure,
// where we will want to check often until it's ready.
const REQUEUE_VT_SEC_SHORT: i32 = 5;

// Amount of time to wait after requeueing a message for an unexpected failure
// that we would want to try again after awhile.
const REQUEUE_VT_SEC_LONG: i32 = 300;

async fn run(metrics: CustomMetrics) -> Result<(), ConductorError> {
    let pg_conn_url =
        env::var("POSTGRES_QUEUE_CONNECTION").expect("POSTGRES_QUEUE_CONNECTION must be set");
    let control_plane_events_queue =
        env::var("CONTROL_PLANE_EVENTS_QUEUE").expect("CONTROL_PLANE_EVENTS_QUEUE must be set");
    let metrics_events_queue =
        env::var("METRICS_EVENTS_QUEUE").expect("METRICS_EVENTS_QUEUE must be set");
    let data_plane_events_queue =
        env::var("DATA_PLANE_EVENTS_QUEUE").expect("DATA_PLANE_EVENTS_QUEUE must be set");
    let data_plane_basedomain =
        env::var("DATA_PLANE_BASEDOMAIN").expect("DATA_PLANE_BASEDOMAIN must be set");
    let backup_archive_bucket =
        env::var("BACKUP_ARCHIVE_BUCKET").expect("BACKUP_ARCHIVE_BUCKET must be set");
    let storage_archive_bucket =
        env::var("STORAGE_ARCHIVE_BUCKET").expect("STORAGE_ARCHIVE_BUCKET must be set");
    let cf_template_bucket: String = env::var("CF_TEMPLATE_BUCKET")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing CF_TEMPLATE_BUCKET");
    let max_read_ct: i32 = env::var("MAX_READ_CT")
        .unwrap_or_else(|_| "100".to_owned())
        .parse()
        .expect("error parsing MAX_READ_CT");
    let is_cloud_formation: bool = env::var("IS_CLOUD_FORMATION")
        .unwrap_or_else(|_| "true".to_owned())
        .parse()
        .expect("error parsing IS_CLOUD_FORMATION");
    let aws_region: String = env::var("AWS_REGION")
        .unwrap_or_else(|_| "us-east-1".to_owned())
        .parse()
        .expect("error parsing AWS_REGION");
    let is_gcp: bool = env::var("IS_GCP")
        .unwrap_or_else(|_| "false".to_owned())
        .parse()
        .expect("error parsing IS_GCP");
    let gcp_project_id: String = env::var("GCP_PROJECT_ID")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing GCP_PROJECT_ID");
    let gcp_project_number: String = env::var("GCP_PROJECT_NUMBER")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing GCP_PROJECT_NUMBER");
    let is_azure: bool = env::var("IS_AZURE")
        .unwrap_or_else(|_| "false".to_owned())
        .parse()
        .expect("error parsing IS_AZURE");
    let azure_storage_account: String = env::var("AZURE_STORAGE_ACCOUNT")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing AZURE_STORAGE_ACCOUNT");
    let azure_subscription_id: String = env::var("AZURE_SUBSCRIPTION_ID")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing AZURE_SUBSCRIPTION_ID");
    // This is necessary for working with multiple resource groups. Example format: cdb-plat-eus-dev
    let azure_resource_group_prefix: String = env::var("AZURE_RESOURCE_GROUP_PREFIX")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing AZURE_RESOURCE_GROUP_PREFIX");
    let azure_region: String = env::var("AZURE_REGION")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing AZURE_REGION");
    let is_loadbalancer_public: bool = env::var("IS_LOADBALANCER_PUBLIC")
        .unwrap_or_else(|_| "true".to_owned())
        .parse()
        .expect("error parsing IS_LOADBALANCER_PUBLIC");
    let storage_class_name: String = env::var("STORAGE_CLASS_NAME")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing STORAGE_CLASS_NAME");
    let is_custom_s3_backup: bool = env::var("IS_CUSTOM_S3_BACKUP")
        .unwrap_or_else(|_| "false".to_owned())
        .parse()
        .expect("error parsing IS_CUSTOM_S3_BACKUP");

    // Custom S3 backup configuration
    let s3_bucket: String = env::var("CUSTOM_S3_BUCKET")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing CUSTOM_S3_BUCKET");
    let s3_endpoint: String = env::var("CUSTOM_S3_ENDPOINT")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing CUSTOM_S3_ENDPOINT");
    let s3_access_key = env::var("CUSTOM_S3_ACCESS_KEY")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing CUSTOM_S3_ACCESS_KEY");
    let s3_secret_key = env::var("CUSTOM_S3_SECRET_KEY")
        .unwrap_or_else(|_| "".to_owned())
        .parse()
        .expect("error parsing CUSTOM_S3_SECRET_KEY");

    // Error and exit if CF_TEMPLATE_BUCKET is not set when IS_CLOUD_FORMATION is enabled
    if is_cloud_formation && cf_template_bucket.is_empty() {
        panic!("CF_TEMPLATE_BUCKET is required when IS_CLOUD_FORMATION is true");
    }

    // Only allow for setting one of IS_CLOUD_FORMATION, IS_GCP, IS_AZURE, or IS_CUSTOM_S3_BACKUP to true
    let cloud_providers = [is_cloud_formation, is_gcp, is_azure, is_custom_s3_backup]
        .iter()
        .filter(|&&x| x)
        .count();

    if cloud_providers > 1 {
        panic!("Only one of IS_CLOUD_FORMATION, IS_GCP, IS_AZURE, or IS_CUSTOM_S3_BACKUP can be set to true");
    }

    // Error and exit if IS_GCP is true and GCP_PROJECT_ID or GCP_PROJECT_NUMBER are not set
    if is_gcp && (gcp_project_id.is_empty() || gcp_project_number.is_empty()) {
        panic!("GCP_PROJECT_ID and GCP_PROJECT_NUMBER must be set if IS_GCP is true");
    }

    // Error and exit if IS_AZURE is true and any of the required Azure environment variables are not set
    if let Err(err) = validate_azure_environment(
        is_azure,
        &azure_storage_account,
        &azure_subscription_id,
        &azure_resource_group_prefix,
        &azure_region,
    ) {
        panic!("{}", err);
    }

    // Connect to pgmq
    let queue = PGMQueueExt::new(pg_conn_url.clone(), 5).await?;
    queue.init().await?;

    // enable pg_partman in the queue -- pass in the connection to the queue to execution
    sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_partman;")
        .execute(&queue.connection)
        .await
        .expect("failed to init pg_partman extension in the queue");

    // Create queues if they do not exist
    queue
        .create_partitioned(&control_plane_events_queue)
        .await?;
    queue.create_partitioned(&data_plane_events_queue).await?;
    queue.create_partitioned(&metrics_events_queue).await?;

    // Infer the runtime environment and try to create a Kubernetes Client
    let client = Client::try_default().await?;

    // Connection Pool
    let db_pool = PgPoolOptions::new()
        .connect(&pg_conn_url)
        .await
        .map_err(|e| {
            error!("Failed to create PG pool: {}", e);
            ConductorError::ConnectionPoolError(e.to_string())
        })?;

    info!("Running database migrations");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");

    log::info!("Database migrations have been successfully applied.");

    // Determine the cloud provider using the builder
    let cloud_provider = CloudProvider::builder()
        .gcp(is_gcp)
        .aws(is_cloud_formation)
        .azure(is_azure)
        .build();

    loop {
        // Read from queue (check for new message)
        // messages that don't fit a CRUDevent will error
        // set visibility timeout to 90 seconds
        let read_msg = queue
            .read::<CRUDevent>(&control_plane_events_queue, 90_i32)
            .await?;
        let read_msg: Message<CRUDevent> = match read_msg {
            Some(message) => {
                info!(
                    "msg_id: {}, enqueued_at: {}, vt: {}",
                    message.msg_id, message.enqueued_at, message.vt
                );
                message
            }
            None => {
                debug!("no messages in queue");
                tokio::time::sleep(time::Duration::from_secs(1)).await;
                continue;
            }
        };

        let org_id = &read_msg.message.org_id;
        let instance_id = &read_msg.message.inst_id;
        let namespace = read_msg.message.namespace.clone();
        info!("{}: Using namespace {}", read_msg.msg_id, &namespace);

        if read_msg.message.event_type != Event::Delete {
            let namespace_already_deleted = match sqlx::query!(
                "SELECT * FROM deleted_instances WHERE namespace = $1;",
                &namespace
            )
            .fetch_optional(&db_pool)
            .await
            {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(e) => {
                    error!("Database query error: {}", e);
                    continue;
                }
            };

            if namespace_already_deleted {
                info!(
                    "{}: Namespace {} marked as deleted, archiving message.",
                    read_msg.msg_id, namespace
                );
                if let Err(e) = queue
                    .archive(&control_plane_events_queue, read_msg.msg_id)
                    .await
                {
                    error!("Failed to archive message: {}", e);
                }
                continue;
            }
        }

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
                org_id: read_msg.message.org_id,
                inst_id: read_msg.message.inst_id,
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

        // Based on message_type in message, create, update, delete CoreDB
        let event_msg: types::StateToControlPlane = match read_msg.message.event_type {
            // every event is for a single namespace
            Event::Create | Event::Update | Event::Restore | Event::Start | Event::Stop => {
                info!("{}: Got create, restore or update event", read_msg.msg_id);

                // (todo: nhudson) in the future move this to be more specific
                // to the event that we are taking action on.  For now just create
                // the stack without checking.

                if read_msg.message.spec.is_none() {
                    error!(
                        "{}: spec is required on create and update events, archiving message",
                        read_msg.msg_id
                    );
                    let _archived = queue
                        .archive(&control_plane_events_queue, read_msg.msg_id)
                        .await?;
                    metrics
                        .conductor_errors
                        .add(&opentelemetry::Context::current(), 1, &[]);
                    continue;
                }
                // spec.expect() should be safe here - since above we continue in loop when it is None
                let msg_spec = read_msg.message.spec.clone().expect("message spec");

                info!("{}: Creating cloudformation template", read_msg.msg_id);

                // Merge backup and service_account_template into spec
                let mut coredb_spec = msg_spec;

                match init_cloud_perms(
                    aws_region.clone(),
                    backup_archive_bucket.clone(),
                    storage_archive_bucket.clone(),
                    cf_template_bucket.clone(),
                    &read_msg,
                    &mut coredb_spec,
                    is_cloud_formation,
                    &client,
                    is_loadbalancer_public,
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

                init_gcp_storage_workload_identity(
                    is_gcp,
                    gcp_project_id.clone(),
                    gcp_project_number.clone(),
                    &read_msg,
                    &mut coredb_spec,
                    backup_archive_bucket.clone(),
                    storage_archive_bucket.clone(),
                )
                .await?;

                init_azure_storage_workload_identity(
                    is_azure,
                    &read_msg,
                    &mut coredb_spec,
                    backup_archive_bucket.clone(),
                    azure_storage_account.clone(),
                    azure_subscription_id.clone(),
                    azure_resource_group_prefix.clone(),
                    azure_region.clone(),
                )
                .await?;

                init_custom_s3_backup_configuration(
                    is_custom_s3_backup,
                    &read_msg,
                    &mut coredb_spec,
                    s3_bucket.clone(),
                    s3_endpoint.clone(),
                    s3_access_key.clone(),
                    s3_secret_key.clone(),
                )
                .await?;

                info!("{}: Creating namespace", read_msg.msg_id);
                // create Namespace
                create_namespace(client.clone(), &namespace, org_id, instance_id).await?;

                info!("{}: Generating spec", read_msg.msg_id);
                let stack_type = match coredb_spec.stack.as_ref() {
                    Some(stack) => stack.name.clone(),
                    None => String::from("NA"),
                };

                include_storage_configuration(
                    storage_archive_bucket.clone(),
                    &read_msg,
                    &mut coredb_spec,
                );

                // If cloud provider is Azure, we need to pass the storage account name to generate_spec
                // so that the storage account URL can be generated for Azure restore scenarios
                let azure_storage_account = match cloud_provider {
                    CloudProvider::Azure => Some(azure_storage_account.as_str()),
                    _ => None,
                };

                let spec = generate_spec(
                    org_id,
                    &stack_type,
                    instance_id,
                    &read_msg.message.data_plane_id,
                    &namespace,
                    &backup_archive_bucket,
                    azure_storage_account,
                    &coredb_spec,
                    &cloud_provider,
                    &storage_class_name,
                )
                .await?;

                info!("{}: Creating or updating spec", read_msg.msg_id);
                // create or update CoreDB
                create_or_update(client.clone(), &namespace, spec).await?;

                // get connection string values from secret

                info!("{}: Getting connection info", read_msg.msg_id);
                let conn_info = match get_pg_conn(
                    client.clone(),
                    &namespace,
                    &data_plane_basedomain,
                    &coredb_spec,
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

                let current_spec = result?;

                let spec_js = serde_json::to_string(&current_spec.spec).unwrap();
                debug!("dbname: {}, current_spec: {:?}", &namespace, spec_js);

                if is_cloud_formation && read_msg.message.event_type == Event::Stop {
                    if let Some(status) = current_spec.clone().status {
                        match status.running {
                            false => {
                                info!("{}: Deleting cloudformation stack", read_msg.msg_id);
                                delete_cloudformation(aws_region.clone(), &namespace).await?;
                            }
                            true => {
                                requeue_short(
                                    &metrics,
                                    &control_plane_events_queue,
                                    &queue,
                                    &read_msg,
                                )
                                .await?;
                                continue;
                            }
                        }
                    }
                }

                let report_event = match read_msg.message.event_type {
                    Event::Create => Event::Created,
                    Event::Update => Event::Updated,
                    Event::Restore => Event::Restored,
                    Event::Start => Event::Started,
                    Event::Stop => Event::StopComplete,
                    _ => unreachable!(),
                };
                types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    org_id: read_msg.message.org_id,
                    inst_id: read_msg.message.inst_id,
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

                if is_cloud_formation {
                    info!("{}: Deleting cloudformation stack", read_msg.msg_id);
                    delete_cloudformation(aws_region.clone(), &namespace).await?;
                }

                if is_gcp {
                    info!(
                        "{}: Deleting GCP storage workload identity binding",
                        read_msg.msg_id
                    );
                    delete_gcp_storage_workload_identity_binding(
                        &gcp_project_id,
                        &gcp_project_number,
                        &backup_archive_bucket,
                        &storage_archive_bucket,
                        &namespace,
                    )
                    .await?;
                }

                if is_azure {
                    info!(
                        "{}: Deleting Azure storage workload identity binding",
                        read_msg.msg_id
                    );
                    delete_azure_storage_workload_identity_binding(
                        &azure_subscription_id,
                        &azure_resource_group_prefix,
                        &namespace,
                    )
                    .await?;
                }

                let insert_query = sqlx::query!(
                    "INSERT INTO deleted_instances (namespace) VALUES ($1) ON CONFLICT (namespace) DO NOTHING",
                    namespace
                );

                match insert_query.execute(&db_pool).await {
                    Ok(_) => info!(
                        "Namespace inserted into deleted_instances table or already exists: {}",
                        &namespace
                    ),
                    Err(e) => error!(
                        "Failed to insert namespace into deleted_instances table: {}",
                        e
                    ),
                }

                // report state
                types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    org_id: read_msg.message.org_id,
                    inst_id: read_msg.message.inst_id,
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
                let msg_enqueued_at = read_msg.enqueued_at;
                match restart_coredb(client.clone(), &namespace, &namespace, msg_enqueued_at).await
                {
                    Ok(_) => {
                        info!("{}: Instance requested to be restarted", read_msg.msg_id);
                    }
                    Err(_) => {
                        error!("{}: Error restarting instance", read_msg.msg_id);
                        requeue_short(&metrics, &control_plane_events_queue, &queue, &read_msg)
                            .await?;
                        continue;
                    }
                };

                let result = get_coredb_error_without_status(client.clone(), &namespace).await;

                let current_resource = match result {
                    Ok(coredb) => {
                        let as_json = serde_json::to_string(&coredb);
                        debug!("dbname: {}, current: {:?}", &namespace, as_json);
                        coredb
                    }
                    Err(_) => {
                        requeue_short(&metrics, &control_plane_events_queue, &queue, &read_msg)
                            .await?;
                        continue;
                    }
                };

                let conn_info = get_pg_conn(
                    client.clone(),
                    &namespace,
                    &data_plane_basedomain,
                    &current_resource.spec,
                )
                .await;

                types::StateToControlPlane {
                    data_plane_id: read_msg.message.data_plane_id,
                    org_id: read_msg.message.org_id,
                    inst_id: read_msg.message.inst_id,
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
            .await?;

        metrics
            .conductor_completed
            .add(&opentelemetry::Context::current(), 1, &[]);

        info!("{}: archived: {:?}", read_msg.msg_id, archived);
    }
}

fn include_storage_configuration(
    storage_archive_bucket: String,
    read_msg: &Message<CRUDevent>,
    coredb_spec: &mut CoreDBSpec,
) {
    if let Some(write_path) = &read_msg.message.backups_write_path {
        let extra_pg_config = PgConfig {
            name: "tembo.storage_bucket_and_path".to_string(),
            value: ConfigValue::Single(format!("{}/{}", storage_archive_bucket, write_path)),
        };
        let mut contains_storage_already = false;
        let mut current_config = coredb_spec.runtime_config.clone().unwrap_or_default();
        for config in current_config.iter() {
            if config.name == "tembo.storage_bucket_and_path" {
                contains_storage_already = true;
            }
        }
        if !contains_storage_already {
            current_config.push(extra_pg_config);
            coredb_spec.runtime_config = Some(current_config);
        }
    }
}

async fn requeue_short(
    metrics: &CustomMetrics,
    control_plane_events_queue: &str,
    queue: &PGMQueueExt,
    read_msg: &Message<CRUDevent>,
) -> Result<(), ConductorError> {
    let _ = queue
        .set_vt::<CRUDevent>(
            control_plane_events_queue,
            read_msg.msg_id,
            REQUEUE_VT_SEC_SHORT,
        )
        .await?;
    metrics.conductor_requeues.add(
        &opentelemetry::Context::current(),
        1,
        &[KeyValue::new("queue_duration", "short")],
    );
    Ok(())
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

    let conductor_enabled = from_env_default("CONDUCTOR_ENABLED", "true");
    let status_reporter_enabled = from_env_default("WATCHER_ENABLED", "true");
    let metrics_reported_enabled = from_env_default("METRICS_REPORTER_ENABLED", "false");

    if conductor_enabled != "false" {
        info!("Starting conductor");
        background_threads_locked.push(tokio::spawn({
            let custom_metrics_copy = custom_metrics.clone();

            async move {
                loop {
                    match run(custom_metrics_copy.clone()).await {
                        Ok(_) => {}
                        Err(ConductorError::PgmqError(pgmq::errors::PgmqError::DatabaseError(
                            Error::PoolTimedOut,
                        ))) => {
                            custom_metrics_copy.clone().conductor_errors.add(
                                &opentelemetry::Context::current(),
                                1,
                                &[],
                            );
                            error!("sqlx PoolTimedOut error in conductor. ending loop");
                            break;
                        }
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
                    tokio::time::sleep(time::Duration::from_secs(1)).await;
                }
            }
        }));
    }

    if status_reporter_enabled != "false" {
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
                    warn!("status_reporter exited, sleeping for 1 second");
                    tokio::time::sleep(time::Duration::from_secs(1)).await;
                }
            }
        }));
    }

    if metrics_reported_enabled != "false" {
        info!("Starting status reporter");
        let custom_metrics_copy = custom_metrics.clone();
        background_threads_locked.push(tokio::spawn(async move {
            let custom_metrics = &custom_metrics_copy;
            if let Err(err) = run_metrics_reporter().await {
                custom_metrics
                    .conductor_errors
                    .add(&opentelemetry::Context::current(), 1, &[]);

                error!("error in metrics_reporter: {err}")
            }

            warn!("metrics_reporter exited, sleeping for 1 second");
            tokio::time::sleep(time::Duration::from_secs(1)).await;
        }));
    }

    std::mem::drop(background_threads_locked);

    let server_port = env::var("PORT")
        .unwrap_or_else(|_| String::from("8080"))
        .parse::<u16>()
        .unwrap_or(8080);

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
    .bind(("0.0.0.0", server_port))?
    .run()
    .await
}

#[allow(clippy::too_many_arguments)]
async fn init_cloud_perms(
    aws_region: String,
    backup_archive_bucket: String,
    storage_archive_bucket: String,
    cf_template_bucket: String,
    read_msg: &Message<CRUDevent>,
    coredb_spec: &mut CoreDBSpec,
    is_cloud_formation: bool,
    _client: &Client,
    is_loadbalancer_public: bool,
) -> Result<(), ConductorError> {
    if !is_cloud_formation {
        return Ok(());
    }

    create_cloudformation(
        aws_region.clone(),
        backup_archive_bucket.clone(),
        storage_archive_bucket.clone(),
        read_msg.message.namespace.clone(),
        read_msg.message.backups_read_path.clone(),
        read_msg.message.backups_write_path.clone(),
        cf_template_bucket,
    )
    .await?;

    // Lookup the CloudFormation stack's role ARN
    let role_arn = lookup_role_arn(aws_region, &read_msg.message.namespace).await?;

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

    // TODO: disable volumesnapshots for now until we can make them work with CNPG
    // Enable VolumeSnapshots for all instances being created
    let volume_snapshot = Some(VolumeSnapshot {
        enabled: false,
        snapshot_class: None,
    });

    let write_path = read_msg
        .message
        .backups_write_path
        .clone()
        .unwrap_or(format!("v2/{}", read_msg.message.namespace));
    let backup = Backup {
        destinationPath: Some(format!("s3://{}/{}", backup_archive_bucket, write_path)),
        encryption: Some(String::from("AES256")),
        retentionPolicy: Some(String::from("30")),
        schedule: Some(generate_cron_expression(&read_msg.message.namespace)),
        s3_credentials: Some(S3Credentials {
            inherit_from_iam_role: Some(true),
            ..Default::default()
        }),
        volume_snapshot,
        ..Default::default()
    };

    coredb_spec.backup = backup;
    coredb_spec.serviceAccountTemplate = service_account_template;

    if is_loadbalancer_public {
        if let Some(ref mut dedicated_networking) = coredb_spec.dedicated_networking {
            dedicated_networking.public = true;
        }
    }

    Ok(())
}

async fn init_gcp_storage_workload_identity(
    is_gcp: bool,
    gcp_project_id: String,
    gcp_project_number: String,
    read_msg: &Message<CRUDevent>,
    coredb_spec: &mut CoreDBSpec,
    backup_archive_bucket: String,
    storage_archive_bucket: String,
) -> Result<(), ConductorError> {
    if !is_gcp {
        return Ok(());
    }

    // Create the Workload Identity binding to the instance's service account and namespace.
    create_gcp_storage_workload_identity_binding(
        &gcp_project_id,
        &gcp_project_number,
        &backup_archive_bucket,
        &storage_archive_bucket,
        &read_msg.message.namespace,
    )
    .await?;

    // Generate Backup spec for CoreDB
    // TODO: disable volumesnapshots for now until we can make them work with CNPG
    // Enable VolumeSnapshots for all instances being created
    let volume_snapshot = Some(VolumeSnapshot {
        enabled: false,
        snapshot_class: None,
    });

    let write_path = read_msg
        .message
        .backups_write_path
        .clone()
        .unwrap_or(format!("v2/{}", read_msg.message.namespace));

    let backup = Backup {
        destinationPath: Some(format!("gs://{}/{}", backup_archive_bucket, write_path)),
        encryption: Some(String::from("AES256")),
        retentionPolicy: Some(String::from("30")),
        schedule: Some(generate_cron_expression(&read_msg.message.namespace)),
        s3_credentials: None,
        azure_credentials: None,
        endpoint_url: None,
        google_credentials: Some(GoogleCredentials {
            gke_environment: Some(true),
            ..Default::default()
        }),
        volume_snapshot,
    };

    coredb_spec.backup = backup;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn init_azure_storage_workload_identity(
    is_azure: bool,
    read_msg: &Message<CRUDevent>,
    coredb_spec: &mut CoreDBSpec,
    backup_archive_bucket: String,
    azure_storage_account: String,
    azure_subscription_id: String,
    azure_resource_group: String,
    azure_region: String,
) -> Result<(), ConductorError> {
    if !is_azure {
        return Ok(());
    }

    let uami_client_id = create_azure_storage_workload_identity_binding(
        &azure_subscription_id,
        &azure_resource_group,
        &azure_region,
        &azure_storage_account,
        &backup_archive_bucket,
        &read_msg.message.namespace,
    )
    .await?;

    // Format ServiceAccountTemplate spec in CoreDBSpec
    use std::collections::BTreeMap;
    let mut annotations: BTreeMap<String, String> = BTreeMap::new();
    annotations.insert(
        "azure.workload.identity/client-id".to_string(),
        uami_client_id,
    );
    let service_account_template = ServiceAccountTemplate {
        metadata: Some(ObjectMeta {
            annotations: Some(annotations),
            ..ObjectMeta::default()
        }),
    };

    // Generate Backup spec for CoreDB
    let volume_snapshot = Some(VolumeSnapshot {
        enabled: false,
        snapshot_class: None,
    });

    let write_path = read_msg
        .message
        .backups_write_path
        .clone()
        .unwrap_or("v2".to_string());

    let backup = Backup {
        destinationPath: Some(format!(
            "https://{}.blob.core.windows.net/{}/{}",
            azure_storage_account, backup_archive_bucket, write_path
        )),
        encryption: Some(String::from("AES256")),
        retentionPolicy: Some(String::from("30")),
        schedule: Some(generate_cron_expression(&read_msg.message.namespace)),
        s3_credentials: None,
        azure_credentials: Some(AzureCredentials {
            connection_string: None,
            inherit_from_azure_ad: Some(true),
            storage_account: None,
            storage_key: None,
            storage_sas_token: None,
        }),
        endpoint_url: None,
        google_credentials: None,
        volume_snapshot,
    };

    coredb_spec.backup = backup;
    coredb_spec.serviceAccountTemplate = service_account_template;

    Ok(())
}

async fn init_custom_s3_backup_configuration(
    is_custom_s3_backup: bool,
    read_msg: &Message<CRUDevent>,
    coredb_spec: &mut CoreDBSpec,
    s3_bucket: String,
    s3_endpoint: String,
    s3_access_key: String,
    s3_secret_key: String,
) -> Result<(), ConductorError> {
    if !is_custom_s3_backup {
        return Ok(());
    }

    let write_path = read_msg
        .message
        .backups_write_path
        .clone()
        .unwrap_or(format!("v2/{}", read_msg.message.namespace));

    // Construct the full S3 destination path
    let full_destination = format!("s3://{}/{}", s3_bucket, write_path);

    // Create S3 credentials configuration
    let s3_credentials = Some(S3Credentials {
        access_key_id: Some(S3CredentialsAccessKeyId(s3_access_key)),
        secret_access_key: Some(S3CredentialsSecretAccessKey(s3_secret_key)),
        region: None,
        inherit_from_iam_role: Some(false),
        session_token: None,
    });

    // Create the backup configuration with default values for encryption and retention
    let backup = Backup {
        destinationPath: Some(full_destination),
        encryption: Some(String::from("AES256")),
        retentionPolicy: Some(String::from("30")),
        schedule: Some(generate_cron_expression(&read_msg.message.namespace)),
        s3_credentials,
        azure_credentials: None,
        endpoint_url: Some(s3_endpoint),
        google_credentials: None,
        volume_snapshot: Some(VolumeSnapshot {
            enabled: false,
            snapshot_class: None,
        }),
    };

    coredb_spec.backup = backup;

    Ok(())
}

fn from_env_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

/// Validates that all required Azure environment variables are set when IS_AZURE is true.
/// Returns Ok(()) if all variables are present and non-empty, or an error message listing missing variables.
fn validate_azure_environment(
    is_azure: bool,
    azure_storage_account: &str,
    azure_subscription_id: &str,
    azure_resource_group_prefix: &str,
    azure_region: &str,
) -> Result<(), String> {
    if !is_azure {
        return Ok(());
    }

    let required_vars = [
        ("AZURE_STORAGE_ACCOUNT", azure_storage_account),
        ("AZURE_SUBSCRIPTION_ID", azure_subscription_id),
        ("AZURE_RESOURCE_GROUP_PREFIX", azure_resource_group_prefix),
        ("AZURE_REGION", azure_region),
    ];

    let missing_vars: Vec<&str> = required_vars
        .iter()
        .filter(|(_, value)| value.is_empty())
        .map(|(name, _)| *name)
        .collect();

    if missing_vars.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "The following required Azure environment variables are empty: {}",
            missing_vars.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_validation() {
        // Test when Azure is disabled
        assert!(validate_azure_environment(false, "", "", "", "").is_ok());

        // Test when Azure is enabled and all variables are present
        assert!(validate_azure_environment(
            true,
            "storage_account",
            "subscription_id",
            "resource_group",
            "region"
        )
        .is_ok());

        // Test when Azure is enabled and variables are missing
        let err =
            validate_azure_environment(true, "", "subscription_id", "resource_group", "region")
                .unwrap_err();
        assert!(err.contains("AZURE_STORAGE_ACCOUNT"));

        // Test multiple missing variables
        let err = validate_azure_environment(true, "", "", "resource_group", "").unwrap_err();
        assert!(err.contains("AZURE_STORAGE_ACCOUNT"));
        assert!(err.contains("AZURE_SUBSCRIPTION_ID"));
        assert!(err.contains("AZURE_REGION"));
    }
}
