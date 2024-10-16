use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use sqlx::{self, Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::authorization;
use crate::config::rewrite_model_request;
use crate::errors::{AuthError, PlatformError};

pub async fn forward_request(
    req: HttpRequest,
    body: web::Json<serde_json::Value>,
    config: web::Data<crate::config::Config>,
    client: web::Data<reqwest::Client>,
    dbclient: web::Data<Arc<PgPool>>,
    cache: web::Data<Arc<RwLock<HashMap<String, bool>>>>,
) -> Result<HttpResponse, PlatformError> {
    let headers = req.headers();
    let x_tembo_org = if let Some(header) = headers.get("X-TEMBO-ORG") {
        header.to_str().unwrap()
    } else {
        return Err(
            AuthError::Forbidden("Missing request header `X-TEMBO-ORG`".to_string()).into(),
        );
    };
    let x_tembo_inst = if let Some(header) = headers.get("X-TEMBO-INSTANCE") {
        header.to_str().unwrap()
    } else {
        return Err(
            AuthError::Forbidden("Missing request header `X-TEMBO-INSTANCE`".to_string()).into(),
        );
    };

    if config.org_auth_enabled {
        let is_valid = authorization::auth_org(x_tembo_org, &cache).await;
        if !is_valid {
            return Err(AuthError::Forbidden("Organization is not authorized".to_string()).into());
        }
    }

    let path = req.uri().path();
    if path.contains("embeddings") {
        return Ok(HttpResponse::BadRequest().body("Embedding generation is not yet supported"));
    }

    let rewrite_request = rewrite_model_request(body.clone(), &config)?;

    let mut new_url = rewrite_request.base_url;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    // log request duration
    let start = std::time::Instant::now();
    let resp = client.post(new_url).json(&body).send().await?;
    let duration = start.elapsed().as_millis() as i32;
    if resp.status().is_success() {
        let llm_resp = resp.json::<serde_json::Value>().await?;
        let model = llm_resp
            .get("model")
            .ok_or_else(|| {
                PlatformError::InvalidQuery("invalid response from model server".to_string())
            })?
            .as_str()
            .ok_or_else(|| {
                PlatformError::InvalidQuery("invalid response from model server".to_string())
            })?;
        let usage: Usage = serde_json::from_value(
            llm_resp
                .get("usage")
                .ok_or_else(|| {
                    PlatformError::InvalidQuery("invalid response from model server".to_string())
                })?
                .clone(),
        )?;
        if let Err(e) =
            insert_data(x_tembo_org, x_tembo_inst, model, usage, duration, &dbclient).await
        {
            log::error!("{}", e);
        }
        Ok(HttpResponse::Ok().json(llm_resp))
    } else {
        let error = resp.text().await?;
        Ok(HttpResponse::BadRequest().body(error))
    }
}

async fn insert_data(
    org: &str,
    isnt: &str,
    model: &str,
    usage: Usage,
    duration_ms: i32,
    con: &Pool<Postgres>,
) -> Result<(), PlatformError> {
    let _r = sqlx::query!(
        "INSERT INTO inference.requests ( organization_id, instance_id, model, prompt_tokens, completion_tokens, duration_ms )
        VALUES ($1, $2, $3, $4, $5, $6)",
        org,
        isnt,
        model,
        usage.prompt_tokens,
        usage.completion_tokens,
        duration_ms
    )
    .execute(con)
    .await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
}
