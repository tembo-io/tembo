//! Custom entrypoint for background running services

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use gateway::config::Config;
use gateway::events_reporter::run_events_reporter;
use log::{info, error};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use sqlx::{PgPool, Postgres, Pool, query, query_scalar};

async fn run_pgmq_maintenance(pool: &Pool<Postgres>) -> Result<(), anyhow::Error> {
    info!("Running PGMQ maintenance");
    
    // Use a configurable timeout for the maintenance function
    let timeout = std::time::Duration::from_secs(30);
    
    let result = tokio::time::timeout(timeout, async {
        // Run maintenance with statement timeout to prevent long-running operations
        sqlx::query("SET statement_timeout = '30s'")
            .execute(pool)
            .await?;
            
        // Run in batches with limit parameter to reduce resource usage
        let result = sqlx::query_scalar::<_, i64>("SELECT run_maintenance(100)")
            .fetch_one(pool)
            .await;
            
        // Reset statement timeout
        sqlx::query("RESET statement_timeout")
            .execute(pool)
            .await?;
            
        result.map_err(|e| anyhow::anyhow!(e))
    }).await;
    
    match result {
        Ok(Ok(count)) => {
            info!("PGMQ maintenance completed successfully, processed {} items", count);
            Ok(())
        },
        Ok(Err(e)) => {
            log::error!("PGMQ maintenance error: {}", e);
            Err(e)
        },
        Err(_) => {
            log::error!("PGMQ maintenance timed out");
            Err(anyhow::anyhow!("PGMQ maintenance timed out"))
        }
    }
}

async fn run_pgmq_maintenance_task(pg_conn: String) {
    let pool = match PgPool::connect(&pg_conn).await {
        Ok(pool) => pool,
        Err(e) => {
            log::error!("Failed to connect to database for PGMQ maintenance: {}", e);
            return;
        }
    };
    
    // Run maintenance every 4 hours to reduce frequency
    let maintenance_interval = Duration::from_secs(4 * 60 * 60);
    let mut interval = tokio::time::interval(maintenance_interval);
    
    loop {
        interval.tick().await;
        
        if let Err(err) = run_pgmq_maintenance(&pool).await {
            log::error!("PGMQ maintenance error: {}", err);
        }
        
        // Allow some time between attempts if there was an error
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let cfg = Config::new().await;

    let background_threads: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let mut background_threads_guard = background_threads.lock().await;

    if cfg.run_billing_reporter {
        info!("Spawning AI billing reporter thread");

        let pg_conn = cfg.pg_conn_str.clone();
        let billing_queue_conn = cfg.billing_queue_conn_str.clone();

        background_threads_guard.push(tokio::spawn(async move {
            loop {
                if let Err(err) =
                    run_events_reporter(pg_conn.clone(), billing_queue_conn.clone()).await
                {
                    log::error!("Tembo AI billing reporter error: {err}");
                    log::info!("Restarting Tembo AI billing reporter in 30 sec");
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }));
    }
    
    // Add PGMQ maintenance thread
    info!("Spawning PGMQ maintenance thread");
    let pg_conn = cfg.pg_conn_str.clone();
    background_threads_guard.push(tokio::spawn(async move {
        run_pgmq_maintenance_task(pg_conn).await;
    }));

    std::mem::drop(background_threads_guard);

    let server_port = std::env::var("PORT")
        .unwrap_or_else(|_| String::from("8080"))
        .parse::<u16>()
        .unwrap_or(8080);

    info!(
        "Starting Tembo AI Billing actix-web server on http://0.0.0.0:{}",
        server_port
    );

    let _ = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(background_threads.clone()))
            .service(web::scope("/health").service(background_threads_running))
    })
    .workers(4)
    .bind(("0.0.0.0", server_port))?
    .run()
    .await;

    Ok(())
}

#[get("/lively")]
pub async fn background_threads_running(
    background_threads: web::Data<Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>>,
) -> impl Responder {
    let background_threads = background_threads.lock().await;

    for thread in background_threads.iter() {
        if thread.is_finished() {
            return HttpResponse::InternalServerError()
                .body("One or more background tasks are not running.");
        }
    }

    HttpResponse::Ok().json("ok")
}
