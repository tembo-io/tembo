use actix_web::{web, App};

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    use actix_web::test;
    use dataplane_webserver::config;
    use dataplane_webserver::routes::health::{lively, ready};
    use dataplane_webserver::routes::{metrics, root};
    use reqwest::Url;

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

        // Test with step parameter
        let query_url = format!("{}&step=5m", query_url);
        let req = test::TestRequest::get()
            .uri(query_url.as_str())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Test with end parameter
        let end = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Failed to get UNIX time")
            .as_secs();
        let query_url = format!("{}&end={}", query_url, end);
        let req = test::TestRequest::get()
            .uri(query_url.as_str())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Test with too many samples (more than 10,000)
        let query_url = format_prometheus_query(url, query, start);
        let query_url = format!("{}&step=1s", query_url);
        let req = test::TestRequest::get()
            .uri(query_url.as_str())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_client_error());

        // Test with end time before start time
        let end = start - 1;
        let query_url = format_prometheus_query(url, query, start);
        let query_url = format!("{}&end={}", query_url, end);
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
