use actix_web::web;

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    use dataplane_webserver::config;
    use dataplane_webserver::routes::health::{lively, ready};
    use dataplane_webserver::routes::{metrics, root};
    use reqwest::Url;
    use actix_web::{test, App, http::StatusCode};
    use dataplane_webserver::routes::secrets::{
        get_secret, get_secret_names, get_secret_names_v1, get_secret_v1, update_postgres_password,
    };
    use serde_json::json;

    #[actix_web::test]
    async fn test_get_secret_names() {
        let app = test::init_service(
            App::new().service(
                web::scope("/{namespace}/secrets")
                    .service(get_secret_names),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test_namespace/secrets")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_secret() {
        let app = test::init_service(
            App::new().service(
                web::scope("/{namespace}/secrets")
                    .service(get_secret),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test_namespace/secrets/app-role")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn test_get_secret_names_v1() {
        let app = test::init_service(
            App::new().service(
                web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(get_secret_names_v1),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/orgs/test_org/instances/test_instance/secrets")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_secret_v1() {
        let app = test::init_service(
            App::new().service(
                web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(get_secret_v1),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/orgs/test_org/instances/test_instance/secrets/app-role")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn test_update_postgres_password() {
        let app = test::init_service(
            App::new().service(
                web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(update_postgres_password),
            ),
        )
        .await;

        let req = test::TestRequest::patch()
            .uri("/api/v1/orgs/test_org/instances/test_instance/secrets/app-role")
            .set_json(&json!({ "password": "newpassword" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn test_probes() {
        env_logger::init();

        let app = test::init_service(
            App::new()
                .service(web::scope("/").service(root::ok))
                .service(web::scope("/health").service(ready).service(lively)),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::get().uri("/health/lively").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::get().uri("/health/ready").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    fn format_prometheus_query(url: &str, query: &str, start: u64) -> String {
        // 24 hours ago, unix time
        let start = start.to_string();
        let start = start.as_str();
        let query_params = vec![("query", query), ("start", start)];
        let url = format!("http://localhost{}", url);
        let url = url.as_str();
        let query_url = Url::parse_with_params(url, &query_params)
            .expect("Failed to format query parameters")
            .to_string();
        return query_url.trim_start_matches("http://localhost").to_string();
    }

    fn format_prometheus_instant_query(url: &str, query: &str) -> String {
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Failed to get UNIX time")
            .as_secs()
            .to_string();
        let query_params = vec![("query", query), ("time", &time)];
        let url = format!("http://localhost{}", url);
        Url::parse_with_params(url.as_str(), &query_params)
            .expect("Failed to format query parameters")
            .to_string()
            .trim_start_matches("http://localhost")
            .to_string()
    }

    #[actix_web::test]
    async fn test_metrics_query_range() {
        let cfg = config::Config::default();
        let http_client = reqwest::Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(cfg.clone()))
                .app_data(web::Data::new(http_client.clone()))
                .service(web::scope("/{namespace}/metrics").service(metrics::query_range)),
        )
        .await;

        let url = "/org-coredb-inst-control-plane-dev/metrics/query_range";
        let query = "(sum by (namespace) (max_over_time(pg_stat_activity_count{namespace=\"org-coredb-inst-control-plane-dev\"}[1h])))";
        // 24 hours ago, unix time
        let start = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Failed to get UNIX time")
            .as_secs()
            - 24 * 60 * 60;
        let query_url = format_prometheus_query(url, query, start);
        let req = test::TestRequest::get()
            .uri(query_url.as_str())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // 25 hours ago, unix time
        let start = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Failed to get UNIX time")
            .as_secs()
            - 25 * 60 * 60;
        let query_url = format_prometheus_query(url, query, start);
        let req = test::TestRequest::get()
            .uri(query_url.as_str())
            .to_request();
        let resp = test::call_service(&app, req).await;
        // It should be a client error to request greater time frame than 1 day
        assert!(resp.status().is_client_error());

        // 1 hour ago, unix time
        let start = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Failed to get UNIX time")
            .as_secs()
            - 60 * 60;
        let query = "(sum by (namespace) (max_over_time(pg_stat_activity_count{namespace=\"org-foobar-inst-control-plane-dev\"}[1h])))";
        let query_url = format_prometheus_query(url, query, start);
        let req = test::TestRequest::get()
            .uri(query_url.as_str())
            .to_request();
        let resp = test::call_service(&app, req).await;
        // It should be a client error if we try to request a namespace we do not own
        assert!(resp.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_metrics_query_instant() {
        let cfg = config::Config::default();
        let http_client = reqwest::Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(cfg.clone()))
                .app_data(web::Data::new(http_client.clone()))
                .service(web::scope("/{namespace}/metrics").service(metrics::query)),
        )
        .await;

        let url = "/org-coredb-inst-control-plane-dev/metrics/query";
        let query =
            "sum(rate(http_requests_total{namespace=\"org-coredb-inst-control-plane-dev\"}[5m]))";
        let query_url = format_prometheus_instant_query(url, query);
        let req = test::TestRequest::get()
            .uri(query_url.as_str())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}
