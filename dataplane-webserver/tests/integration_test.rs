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
    use dataplane_webserver::secrets::types::PasswordString;

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

    fn format_secrets_query_url(org_id: &str, instance_id: &str) -> String {
        let url = format!(
            "http://localhost/api/v1/orgs/{}/instances/{}/secrets",
            org_id, instance_id
        );
        Url::parse(url.as_str())
            .expect("Failed to format URL")
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
        print!("{:?}",req);
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_secrets_v1() {
        let cfg = config::Config::default();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(cfg.clone()))
                .service(web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(secrets::get_secret_names_v1)),
        )
        .await;

        // Use the format function to generate the URL
        let url = format_secrets_query_url("org_2X6FKW5Ng0Tfq08by32pfx5GNaq", "inst_1723054469096_bNPpJJ_3");
        let req = test::TestRequest::get().uri(url.as_str()).to_request();
        let resp = test::call_service(&app, req).await;

        // Assert that the status is success
        assert!(resp.status().is_success());
    }

    fn format_secret_name_query_url(org_id: &str, instance_id: &str, secret_name: &str) -> String {
        let url = format!(
            "http://localhost/api/v1/orgs/{}/instances/{}/secrets/{}",
            org_id, instance_id, secret_name
        );
        Url::parse(url.as_str())
            .expect("Failed to format URL")
            .to_string()
            .trim_start_matches("http://localhost")
            .to_string()
    }

    fn format_update_postgres_password(org_id: &str, instance_id: &str, secret_name: &str) -> String {
        let url = format!(
            "http://localhost/api/v1/orgs/{}/instances/{}/secrets/{}",
            org_id, instance_id, secret_name
        );
        Url::parse(url.as_str())
            .expect("Failed to format URL")
            .to_string()
            .trim_start_matches("http://localhost")
            .to_string()
    }
    

    #[actix_web::test]
    async fn test_get_secret_name() {
        let cfg = config::Config::default();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(cfg.clone()))
                .service(web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(secrets::get_secret_v1)),
        )
        .await;

        // Test valid request
        let url = format_secret_name_query_url(
            "org_2a8iaiQUhH66QjY11FH9lgKxyBr",
            "inst_1721747309879_2NpSYd_23",
            "readonly-role",
        );
        let req = test::TestRequest::get().uri(url.as_str()).insert_header(("Authorization", "Bearer eyJhbGciOiJSUzI1NiIsImNhdCI6ImNsX0I3ZDRQRDIyMkFBQSIsImtpZCI6Imluc18yTnh4R3NWRmMzZGRCeGZWZUo0UjU1dzFsVEciLCJ0eXAiOiJKV1QifQ.eyJhenAiOiJodHRwczovL2Nsb3VkLmNkYi1kZXYuY29tIiwiZXhwIjoxNzI0MTA1Njg2LCJpYXQiOjE3MjQwMTkyODYsImlzcyI6Imh0dHBzOi8vZXZvbHZpbmctYmxvd2Zpc2gtNzMuY2xlcmsuYWNjb3VudHMuZGV2IiwianRpIjoiOWNkZTUyNjE2YzY5NmU5M2E0ZDgiLCJuYmYiOjE3MjQwMTkyODEsIm9yZ19pZCI6Im9yZ18yYThpYWlRVWhINjZRalkxMUZIOWxnS3h5QnIiLCJvcmdfcm9sZSI6ImFkbWluIiwib3JnX3NsdWciOiJqb3NoIiwib3JnYW5pemF0aW9ucyI6eyJvcmdfMmE4aWFpUVVoSDY2UWpZMTFGSDlsZ0t4eUJyIjoiYWRtaW4ifSwic2lkIjoiYXBpLXRva2VuIiwic3ViIjoidXNlcl8yYThpWThjR0trVTBFbmwxYkdVVVpDT3UwNm8ifQ.s3K2DszKpSoleghb0uFUJVbAy0VswYIC3cLFSFkHT742eh941wXb79b6Ay0pnY9RtnL3BF8vCFaPuK2ZIZH3HOtEiFM26X1MCA1tF1BI74pwKOn8MPNdT5Rg2eoDTgkqgGNrwhZaZON96LlruNWTkSrpRa3jdvNji_yL8hmCpZ8uqcO9Mqq2xQZlzw2C1Tvmo6BuWjvg4uswy5_wFURX48w25CJbe9msn5NVmXBoyuiTXqzc6yQoUX0fTDIgAMWCiNlnfpL7rKxvN5Sa8fzB190cUYzPcmaQwPb9bPv2ij6cYI3RhPxxNaQdqvmUjlzAzv09K3Mi24YzUIHqPPqCLw")).to_request();
        let resp = test::call_service(&app, req).await;

        // Extract the body
        let body = test::read_body(resp).await;

        // Print the body
        println!("{:?}", body);
        panic!("Testing out");


        }

    #[actix_web::test]
    async fn test_update_secret() {
        let cfg = config::Config::default();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(cfg.clone()))
                .service(web::scope("/api/v1/orgs/{org_id}/instances/{instance_id}")
                    .service(secrets::update_postgres_password)),
        )
        .await;

        // Test valid request
        let url = format_update_postgres_password(
            "org_2a8iaiQUhH66QjY11FH9lgKxyBr",
            "inst_1721747309879_2NpSYd_23",
            "readonly-role",
        );
        let req = test::TestRequest::get().uri(url.as_str()).insert_header(("Authorization", "Bearer eyJhbGciOiJSUzI1NiIsImNhdCI6ImNsX0I3ZDRQRDIyMkFBQSIsImtpZCI6Imluc18yTnh4R3NWRmMzZGRCeGZWZUo0UjU1dzFsVEciLCJ0eXAiOiJKV1QifQ.eyJhenAiOiJodHRwczovL2Nsb3VkLmNkYi1kZXYuY29tIiwiZXhwIjoxNzI0MTA1Njg2LCJpYXQiOjE3MjQwMTkyODYsImlzcyI6Imh0dHBzOi8vZXZvbHZpbmctYmxvd2Zpc2gtNzMuY2xlcmsuYWNjb3VudHMuZGV2IiwianRpIjoiOWNkZTUyNjE2YzY5NmU5M2E0ZDgiLCJuYmYiOjE3MjQwMTkyODEsIm9yZ19pZCI6Im9yZ18yYThpYWlRVWhINjZRalkxMUZIOWxnS3h5QnIiLCJvcmdfcm9sZSI6ImFkbWluIiwib3JnX3NsdWciOiJqb3NoIiwib3JnYW5pemF0aW9ucyI6eyJvcmdfMmE4aWFpUVVoSDY2UWpZMTFGSDlsZ0t4eUJyIjoiYWRtaW4ifSwic2lkIjoiYXBpLXRva2VuIiwic3ViIjoidXNlcl8yYThpWThjR0trVTBFbmwxYkdVVVpDT3UwNm8ifQ.s3K2DszKpSoleghb0uFUJVbAy0VswYIC3cLFSFkHT742eh941wXb79b6Ay0pnY9RtnL3BF8vCFaPuK2ZIZH3HOtEiFM26X1MCA1tF1BI74pwKOn8MPNdT5Rg2eoDTgkqgGNrwhZaZON96LlruNWTkSrpRa3jdvNji_yL8hmCpZ8uqcO9Mqq2xQZlzw2C1Tvmo6BuWjvg4uswy5_wFURX48w25CJbe9msn5NVmXBoyuiTXqzc6yQoUX0fTDIgAMWCiNlnfpL7rKxvN5Sa8fzB190cUYzPcmaQwPb9bPv2ij6cYI3RhPxxNaQdqvmUjlzAzv09K3Mi24YzUIHqPPqCLw")).set_json(&PasswordString {
            password: "aerthaergeargaergaergaergaergaer".to_string(),  // Example new password
        }).to_request();
        let resp = test::call_service(&app, req).await;

        // Extract the body
        let body = test::read_body(resp).await;

        // Print the body
        println!("{:?}", body);
        panic!("Testing out");


        }
    }

