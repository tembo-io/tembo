use crate::{config, metrics};

use crate::metrics::types::RangeQuery;
use actix_web::{get, web, Error, HttpRequest, HttpResponse};

use reqwest::Client;

#[utoipa::path(
    context_path = "/{namespace}/metrics",
    params(
        ("namespace" = String, Path, example="org-coredb-inst-control-plane-dev", description = "Instance namespace"),
        ("query" = inline(String), Query, example="(sum by (namespace) (max_over_time(pg_stat_activity_count{namespace=\"org-coredb-inst-control-plane-dev\"}[1h])))", description = "PromQL range query, must include a 'namespace' label matching the query path"),
        ("start" = inline(u64), Query, example="1686780828", description = "Range start, unix timestamp"),
        ("end" = inline(Option<u64>), Query, example="1686862041", description = "Range end, unix timestamp. Default is now."),
        ("step" = inline(Option<String>), Query, example="60s", description = "Step size duration string, defaults to 60s"),
    ),
    responses(
        (status = 200, description = "Success range query to Prometheus, please see Prometheus documentation for response format details. https://prometheus.io/docs/prometheus/latest/querying/api/#range-queries", body = Value,
        example = json!({"status":"success","data":{"resultType":"matrix","result":[{"metric":{"__name__":"up","job":"prometheus","instance":"localhost:9090"},"values":[[1435781430.781,"1"],[1435781445.781,"1"],[1435781460.781,"1"]]},{"metric":{"__name__":"up","job":"node","instance":"localhost:9091"},"values":[[1435781430.781,"0"],[1435781445.781,"0"],[1435781460.781,"1"]]}]}})
        ),
        (status = 400, description = "Parameters are missing or incorrect"),
        (status = 403, description = "Not authorized for query"),
        (status = 422, description = "Incorrectly formatted query"),
        (status = 504, description = "Request timed out on metrics backend"),
    )
)]
#[get("/query_range")]
pub async fn query_range(
    cfg: web::Data<config::Config>,
    http_client: web::Data<Client>,
    _req: HttpRequest,
    range_query: web::Query<RangeQuery>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, Error> {
    let (namespace,) = path.into_inner();

    Ok(metrics::query_prometheus(cfg, http_client, range_query, namespace).await)
}
