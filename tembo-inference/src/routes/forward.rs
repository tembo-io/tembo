use std::str::FromStr;

use actix_web::{http::Method, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::{self, Pool, Postgres};
use url::Url;

use crate::errors::{AuthError, PlatformError};

pub async fn forward_request(
    req: HttpRequest,
    method: Method,
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

    let mut new_url = Url::parse(&format!("http://localhost:8000")).unwrap();
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    // For now, only POST is supported
    let resp = match client.post(new_url).json(&body).send().await {
        Ok(resp) => {
            let llm_resp = resp.json::<serde_json::Value>().await?;
            let model = llm_resp.get("model").unwrap().as_str().unwrap();
            let usage: Usage = serde_json::from_value(llm_resp.get("usage").unwrap().clone())?;
            if let Err(e) = insert_data(x_tembo, model, usage, &dbclient).await {
                log::error!("{}", e);
            }
            llm_resp
        }
        Err(e) => Err(e)?,
    };

    Ok(HttpResponse::Ok().json(resp))
}

// Function to insert data into Postgres
async fn insert_data(
    org: &str,
    model: &str,
    usage: Usage,
    con: &Pool<Postgres>,
) -> Result<(), PlatformError> {
    let _r = sqlx::query!(
        "INSERT INTO inference.requests ( organization_id, model, prompt_tokens, completion_tokens )
        VALUES ($1, $2, $3, $4)",
        org,
        model,
        usage.prompt_tokens,
        usage.completion_tokens
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
