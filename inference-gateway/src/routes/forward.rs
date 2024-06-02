use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::{self, Pool, Postgres};

use crate::errors::{AuthError, PlatformError};

pub async fn forward_request(
    req: HttpRequest,
    body: web::Json<serde_json::Value>,
    config: web::Data<crate::config::Config>,
    client: web::Data<reqwest::Client>,
    dbclient: web::Data<Pool<Postgres>>,
) -> Result<HttpResponse, PlatformError> {
    let headers = req.headers();
    let x_tembo = if let Some(header) = headers.get("X-TEMBO-ORG") {
        header.to_str().unwrap()
    } else {
        return Err(AuthError::Forbidden("Missing request headers".to_string()).into());
    };

    let path = req.uri().path();

    let mut new_url = config.llm_service_host_port.clone();
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
        if let Err(e) = insert_data(x_tembo, model, usage, duration, &dbclient).await {
            log::error!("{}", e);
        }
        Ok(HttpResponse::Ok().json(llm_resp))
    } else {
        let error = resp.text().await?;
        Ok(HttpResponse::BadRequest().body(error))
    }
}

// Function to insert data into Postgres
async fn insert_data(
    org: &str,
    model: &str,
    usage: Usage,
    duration_ms: i32,
    con: &Pool<Postgres>,
) -> Result<(), PlatformError> {
    let _r = sqlx::query!(
        "INSERT INTO inference.requests ( organization_id, model, prompt_tokens, completion_tokens, duration_ms )
        VALUES ($1, $2, $3, $4, $5)",
        org,
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
