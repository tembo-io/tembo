use actix_web::{post, web, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sqlx::Column;
use sqlx::Pool;
use sqlx::Postgres;
use sqlx::Row;

#[derive(Deserialize)]
struct SqlRequest {
    query: String,
    params: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[post("/sql")]
async fn sql_endpoint(pool: web::Data<Pool<Postgres>>, req: web::Json<SqlRequest>) -> HttpResponse {
    let mut conn = match pool.acquire().await {
        Ok(conn) => conn,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            })
        }
    };

    let mut query = sqlx::query(&req.query);

    for param in &req.params {
        query = query.bind(param);
    }

    let result = query.fetch_all(&mut *conn).await;

    match result {
        Ok(rows) => {
            // Extract column names
            let columns: Vec<String> = rows[0]
                .columns()
                .iter()
                .map(|col| col.name().to_string())
                .collect();

            // Convert rows to array of arrays
            let array_data: Vec<Vec<JsonValue>> = rows
                .iter()
                .map(|row| {
                    columns
                        .iter()
                        .map(|col| {
                            let value: JsonValue =
                                row.try_get(col.as_str()).unwrap_or(JsonValue::Null);
                            value
                        })
                        .collect()
                })
                .collect();

            let raw_data: Vec<JsonMap<String, JsonValue>> = rows
                .iter()
                .map(|row| {
                    columns
                        .iter()
                        .filter_map(|col| {
                            let value: JsonValue =
                                row.try_get(col.as_str()).unwrap_or(JsonValue::Null);
                            Some((col.clone(), value))
                        })
                        .collect()
                })
                .collect();

            HttpResponse::Ok().json(json!({
                "raw": raw_data,
                "cleaned": array_data,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}
