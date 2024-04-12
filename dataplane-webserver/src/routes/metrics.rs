use crate::{config, metrics};

use crate::metrics::types::{InstantQuery, RangeQuery};
use actix_web::{get, web, Error, HttpRequest, HttpResponse};

use reqwest::Client;

#[utoipa::path(
    context_path = "/{namespace}/metrics",
    params(
        ("namespace" = String, Path, example="org-coredb-inst-control-plane-dev", description = "Instance namespace"),
        ("query" = inline(String), Query, example="(sum by (namespace) (max_over_time(cnpg_backends_total{namespace=\"org-coredb-inst-control-plane-dev\"}[1h])))", description = "PromQL range query, must include a 'namespace' label matching the query path"),
        ("start" = inline(u64), Query, example="1686780828", description = "Range start, unix timestamp"),
        ("end" = inline(Option<u64>), Query, example="1686862041", description = "Range end, unix timestamp. Default is now."),
        ("step" = inline(Option<String>), Query, example="60s", description = "Step size duration string, defaults to 60s"),
    ),
    responses(
        (status = 200, description = "Success range query to Prometheus, please see Prometheus documentation for response format details. https://prometheus.io/docs/prometheus/latest/querying/api/#range-queries", body = Value,
        example = json!({
            "data": {
                "result": [
                    {
                        "metric": {
                            "namespace": "org-uc-ceas-inst-set"
                        },
                        "values": [
                            [
                                1435781430,
                                "2"
                            ],
                            [
                                1435781445,
                                "2"
                            ]
                        ]
                    }
                ],
                "resultType": "matrix"
            },
            "status": "success"
        }),
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

#[utoipa::path(
    context_path = "/{namespace}/metrics",
    params(
        ("namespace" = String, Path, example="org-coredb-inst-control-plane-dev", description = "Instance namespace"),
        ("query" = inline(String), Query, example="(sum by (namespace) (max_over_time(cnpg_backends_total{namespace=\"org-coredb-inst-control-plane-dev\"}[1h])))", description = "PromQL range query, must include a 'namespace' label matching the query path"),
    ),
    responses(
        (status = 200, description = "Success range query to Prometheus, please see Prometheus documentation for response format details. https://prometheus.io/docs/prometheus/latest/querying/api/#instant-queries", body = Value,
        example = json!({
            "data": {
                "result": [
                    {
                        "metric": {
                            "namespace": "org-coredb-inst-control-plane-dev"
                        },
                        "value": [
                            1435781430,
                            "2"
                        ]
                    }
                ],
                "resultType": "vector"
            },
            "status": "success"
        }),
        ),
        (status = 400, description = "Parameters are missing or incorrect"),
        (status = 403, description = "Not authorized for query"),
        (status = 422, description = "Incorrectly formatted query"),
        (status = 504, description = "Request timed out on metrics backend"),
    )
)]
#[get("/query")]
pub async fn query(
    cfg: web::Data<config::Config>,
    http_client: web::Data<Client>,
    instant_query: web::Query<InstantQuery>,
    _req: HttpRequest,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, Error> {
    let (namespace,) = path.into_inner();

    Ok(metrics::query_prometheus_instant(cfg, http_client, instant_query, namespace).await)
}
