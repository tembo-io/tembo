use actix_web::web;

use crate::routes;
use crate::{config, db, validation};

use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

pub fn webserver_routes(configuration: &mut web::ServiceConfig) {
    configuration
        .service(routes::health::ready)
        .service(routes::health::lively)
        .default_service(web::to(routes::forward::forward_request));
}

#[derive(Debug, Clone)]
pub struct ServerStartUpConfig {
    pub cfg: config::Config,
    pub pool: Arc<Pool<Postgres>>,
    pub validation_cache: Arc<RwLock<HashMap<String, bool>>>,
    pub http_client: reqwest::Client,
}

pub async fn webserver_startup_config() -> ServerStartUpConfig {
    let cfg = config::Config::new().await;
    let dbclient: Pool<Postgres> = db::connect(&cfg.pg_conn_str, 4)
        .await
        .expect("Failed to connect to database");
    sqlx::migrate!("./migrations")
        .run(&dbclient)
        .await
        .expect("Failed to run migrations");
    let pool = Arc::new(dbclient);
    let http_client: reqwest::Client = reqwest::Client::new();
    let validation_cache = Arc::new(RwLock::new(HashMap::<String, bool>::new()));

    if cfg.org_validation_enabled {
        let cache_refresher = validation_cache.clone();
        let pool_for_bg_task = pool.clone();
        actix_rt::spawn(async move {
            loop {
                match validation::refresh_cache(&pool_for_bg_task, &cache_refresher).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Failed to refresh cache: {:?}", e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(
                    cfg.org_validation_cache_refresh_interval_sec,
                ))
                .await;
            }
        });
    }

    ServerStartUpConfig {
        cfg,
        pool,
        validation_cache,
        http_client,
    }
}
