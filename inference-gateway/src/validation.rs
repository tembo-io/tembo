use anyhow::Result;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn refresh_cache(
    pool: &PgPool,
    cache: &Arc<RwLock<HashMap<String, bool>>>,
) -> Result<(), sqlx::Error> {
    let mut new_cache = HashMap::new();
    let rows = sqlx::query!("SELECT org_id, valid FROM inference.org_validation")
        .fetch_all(pool)
        .await?;

    log::debug!("Refreshing cache with {} rows", rows.len());
    for row in rows {
        new_cache.insert(row.org_id, row.valid);
    }

    let mut cache_write = cache.write().await;
    *cache_write = new_cache;

    Ok(())
}

/// checks if org's is flagged as validated
pub async fn validate_org(org_id: &str, cache: &Arc<RwLock<HashMap<String, bool>>>) -> bool {
    let cache_read = cache.read().await;
    match cache_read.get(org_id) {
        Some(valid) => *valid,
        None => false,
    }
}
