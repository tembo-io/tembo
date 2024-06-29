use actix_web::web;

use crate::routes;
use crate::{authorization, config, db};

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
    pub auth_cache: Arc<RwLock<HashMap<String, bool>>>,
    pub http_client: reqwest::Client,
}

pub async fn webserver_startup_config(cfg: config::Config) -> ServerStartUpConfig {
    let dbclient: Pool<Postgres> = db::connect(&cfg.pg_conn_str, cfg.server_workers as u32)
        .await
        .expect("Failed to connect to database");
    sqlx::migrate!("./migrations")
        .run(&dbclient)
        .await
        .expect("Failed to run migrations");
    let pool = Arc::new(dbclient);
    let http_client: reqwest::Client = reqwest::Client::new();
    let auth_cache = Arc::new(RwLock::new(HashMap::<String, bool>::new()));

    if cfg.org_auth_enabled {
        log::info!("Starting background task to refresh org auth cache");
        let cache_refresher = auth_cache.clone();
        let pool_for_bg_task = pool.clone();
        actix_rt::spawn(async move {
            loop {
                match authorization::refresh_cache(&pool_for_bg_task, &cache_refresher).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Failed to refresh cache: {:?}", e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(cfg.org_auth_cache_refresh_interval_sec))
                    .await;
            }
        });
    } else {
        log::info!("Org auth is disabled");
    }

    ServerStartUpConfig {
        cfg,
        pool,
        auth_cache,
        http_client,
    }
}
