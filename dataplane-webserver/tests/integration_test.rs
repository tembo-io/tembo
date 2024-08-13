use actix_web::{web, App};

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    use actix_web::test;
    use dataplane_webserver::config;
    use dataplane_webserver::routes::health::{lively, ready};
    use dataplane_webserver::routes::{metrics, root, secrets};
    use reqwest::Url;
    use serde_json::{Value,json};

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

    #[actix_web::test]
    async fn test_get_secret_v1() {
        let cfg = config::Config::default();
        let http_client = reqwest::Client::builder()
            .build()
            .expect("Failed to create HTTP client");
    
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(cfg.clone()))
                .app_data(web::Data::new(http_client.clone()))
                .service(
                    web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                        .service(secrets::get_secret_v1)
                ),
        )
        .await;
    
        let jwt = "Bearer eyJhbGciOiJSUzI1NiIsImNhdCI6ImNsX0I3ZDRQRDIyMkFBQSIsImtpZCI6Imluc18yUDJhR2Ezb1ZkZGVISmtpeG43bXdlYXpNaHciLCJ0eXAiOiJKV1QifQ.eyJhenAiOiJodHRwczovL2Nsb3VkLnRlbWJvLmlvIiwiZXhwIjoxNzIzNTk5Mjk0LCJpYXQiOjE3MjM1MTI4OTQsImlzcyI6Imh0dHBzOi8vY2xlcmsudGVtYm8uaW8iLCJqdGkiOiI1YmFjYTViZTIzYTU3OTQ1NzBjYyIsIm5iZiI6MTcyMzUxMjg4OSwib3JnX2lkIjoib3JnXzJYNkZLVzVOZzBUZnEwOGJ5MzJwZng1R05hcSIsIm9yZ19yb2xlIjoiYWRtaW4iLCJvcmdfc2x1ZyI6InVjIiwib3JnYW5pemF0aW9ucyI6eyJvcmdfMlg2RktXNU5nMFRmcTA4YnkzMnBmeDVHTmFxIjoiYWRtaW4ifSwic2lkIjoiYXBpLXRva2VuIiwic3ViIjoidXNlcl8yWDZGSjJKbGtXdUpjdkdNYm5tNmJkYU5Ld1cifQ.T8uGuzwAkU5rfJrT08H7MBWMOZ86Cqw41RvjQ7pPFRMRiAyA7zTRvxOPWjs3TwSzNDTFr9lVeiIfSJ6RHDKmxYfCrFzipoylDuGmWukgOBRZzsitjfixPX6eC0h4AGvDVEoMPHxMes-GsO9XNdx-PgRnvYwEuQ6aSmYs4BS_YSMRNNG2AvavvEah4gzYWkC0v6ubhPy86DR35CIUkEniHaqz6RprYe2dW8QceLZ9YQLIOL3SEDjiLXBIs29hJMXM-b1TewcM4OTd8Xc6UDKOyPhsCmEH5SsHo7TepRQLODd_3FfmNwC7Mbu2dP4YbK_RAS88EhtgQ98IfPCdtBQG4g";
        let org_id = "org_2X6FKW5Ng0Tfq08by32pfx5GNaq";
        let instance_id = "inst_1723054469096_bNPpJJ_3";
    
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/orgs/{}/instances/{}/secrets", org_id, instance_id))
            .insert_header(("Authorization", jwt))
            .to_request();
    
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
    
        // Optionally, you can check the response body
        let body: Value = test::read_body_json(resp).await;
        
        // Add assertions based on the expected response structure
        assert!(body.is_array());
        assert!(!body.as_array().unwrap().is_empty());
    
        // You might want to check for specific secrets if you know they should be present
        let secret_names: Vec<String> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect();
    
        assert!(secret_names.contains(&"app-role".to_string()));
        assert!(secret_names.contains(&"readonly-role".to_string()));
        assert!(secret_names.contains(&"superuser-role".to_string()));
        assert!(secret_names.contains(&"certificate".to_string()));
    }


}
