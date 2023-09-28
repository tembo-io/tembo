extern crate rpassword;

use reqwest::cookie::Jar;
use reqwest::header;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use rpassword::read_password;
use serde_json::Value;
use simplelog::*;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::io::Write;
use std::sync::Arc;

const CLERK_URL: &str = "https://clerk.tembo.io";
const ORIGIN_URL: &str = "https://accounts.tembo.io";
const CLERK_SIGN_IN_SLUG: &str = "/v1/client/sign_ins?_clerk_js_version=4.53.0";

pub struct AuthClient {}

impl AuthClient {
    pub fn authenticate() -> Result<String, Box<dyn Error>> {
        println!("Please enter the email address for a service user (https://tembo.io/docs/tembo-cloud/api):");
        let user = Self::get_input();

        println!("Please enter the password for the Tembo service user:");
        std::io::stdout().flush().unwrap();
        let password = read_password().unwrap();

        let clerk_url = CLERK_URL;
        let client = Self::client();

        match Self::create_sign_in(&client, clerk_url, &user, &password) {
            Ok(token) => {
                let sign_in_token = token;
                // TODO: match in case this fails
                let session_id =
                    Self::attempt_first_factor(&client, clerk_url, &sign_in_token, &password)
                        .unwrap();

                let jwt = Self::get_expiring_api_token(&client, clerk_url, &session_id).unwrap();

                Ok(jwt)
            }
            Err(e) => {
                // TODO: remind users to use service accounts, not their every day user account
                error!("there was an error signing in: {}", e);
                Err(e)
            }
        }
    }

    fn get_input() -> String {
        let mut this_input = String::from("");

        io::stdin()
            .read_line(&mut this_input)
            .expect("Failed to read line");
        this_input.trim().to_string()
    }

    fn client() -> reqwest::blocking::Client {
        let jar = Jar::default();

        reqwest::blocking::Client::builder()
            .cookie_store(true)
            .cookie_provider(Arc::new(jar))
            .build()
            .unwrap()
    }

    fn headers() -> HeaderMap {
        vec![
            (
                header::CONTENT_TYPE,
                "application/x-www-form-urlencoded".parse().unwrap(),
            ),
            (header::ORIGIN, ORIGIN_URL.parse().unwrap()),
        ]
        .into_iter()
        .collect()
    }

    fn create_sign_in(
        client: &reqwest::blocking::Client,
        url: &str,
        user: &str,
        _pw: &str,
    ) -> Result<String, Box<dyn Error>> {
        let request_url = format!("{}/{}", url, CLERK_SIGN_IN_SLUG);

        let mut map = HashMap::new();
        map.insert("identifier", user);

        let req = client.post(request_url).headers(Self::headers()).form(&map);
        let res = req.send()?;

        match res.status() {
            StatusCode::OK => {
                let json: Value = res.json().unwrap_or("{}".into());
                let response_id = json["response"]["id"].as_str();

                Ok(response_id.unwrap().to_string())
            }
            status_code if status_code.is_client_error() => {
                info!("{}", res.text()?);
                Err(From::from("Client error"))
            }
            _ => Err(From::from("Client error")),
        }
    }

    fn attempt_first_factor(
        client: &reqwest::blocking::Client,
        url: &str,
        id: &str,
        pw: &str,
    ) -> Result<String, Box<dyn Error>> {
        let request_url = format!(
            "{}/v1/client/sign_ins/{}/attempt_first_factor?_clerk_js_version=4.53.0",
            url, id
        );

        let mut map = HashMap::new();
        map.insert("strategy", "password");
        map.insert("password", pw);

        let req = client.post(request_url).headers(Self::headers()).form(&map);
        let res = req.send()?;

        match res.status() {
            StatusCode::OK => (),
            status_code if status_code.is_client_error() => {
                error!("client error:");
                error!("{}", &res.text()?);
                return Err(From::from("Client error"));
            }
            _ => (),
        };

        let json: &Value = &res.json()?;
        let session_id = json["client"]["sessions"][0]["id"]
            .as_str()
            .ok_or("Failed to parse jwt")?
            .to_string();

        Ok(session_id)
    }

    fn get_expiring_api_token(
        client: &reqwest::blocking::Client,
        url: &str,
        session_id: &str,
    ) -> Result<String, Box<dyn Error>> {
        println!(
            "Please enter the number of days you would like this token to valid for [1, 30, 365]:"
        );
        let days = Self::get_input();

        let request_url = format!(
            "{}/v1/client/sessions/{}/tokens/api-token-{}-days?_clerk_js_version=4.53.0",
            url, session_id, days
        );

        let req = client.post(request_url).headers(Self::headers());
        let res = req.send()?;

        match res.status() {
            StatusCode::OK => (),
            status_code if status_code.is_client_error() => {
                error!("- client error:");
                error!("- {}", &res.text()?);
                return Err(From::from("Client error"));
            }
            _ => (),
        };

        let json: &Value = &res.json()?;
        let jwt = json["jwt"]
            .as_str()
            .ok_or("Failed to parse jwt")?
            .to_string();

        Ok(jwt)
    }
}
