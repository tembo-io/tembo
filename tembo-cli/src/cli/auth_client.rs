extern crate rpassword;

use crate::cli::cloud_account::CloudAccount;
use crate::cli::config::Config;
use crate::Result;
use anyhow::{bail, Context};
use chrono::prelude::*;
use clap::ArgMatches;
use reqwest::cookie::Jar;
use reqwest::header;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use rpassword::read_password;
use serde_json::Value;
use simplelog::*;
use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::sync::Arc;

const CLERK_URL: &str = "https://clerk.tembo.io";
const ORIGIN_URL: &str = "https://accounts.tembo.io";
const CLERK_SIGN_IN_SLUG: &str = "/v1/client/sign_ins?_clerk_js_version=4.53.0";

pub struct AuthClient {}

impl AuthClient {
    pub fn authenticate(args: &ArgMatches) -> Result<String> {
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
                    Self::attempt_first_factor(&client, clerk_url, &sign_in_token, &password, args)
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
    ) -> Result<String> {
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
                bail!("Client error")
            }
            _ => bail!("Client error"),
        }
    }

    fn attempt_first_factor(
        client: &reqwest::blocking::Client,
        url: &str,
        id: &str,
        pw: &str,
        args: &ArgMatches,
    ) -> Result<String> {
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
                let error = res.text()?;
                error!("client error:");
                error!("{error}");
                bail!("Client error: {error}");
            }
            _ => (),
        };

        let json: &Value = &res.json()?;
        let session_id = json["client"]["sessions"][0]["id"]
            .as_str()
            .with_context(|| "Failed to parse jwt")?
            .to_string();

        // set or update the user's org ids
        let _ = Self::set_org_ids(json.clone(), args);

        Ok(session_id)
    }

    fn get_expiring_api_token(
        client: &reqwest::blocking::Client,
        url: &str,
        session_id: &str,
    ) -> Result<String> {
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
                bail!("Client error");
            }
            _ => (),
        };

        let json: &Value = &res.json()?;
        let jwt = json["jwt"]
            .as_str()
            .with_context(|| "Failed to parse jwt")?
            .to_string();

        Ok(jwt)
    }

    // stores the user's organization id(s) in the config
    fn set_org_ids(json: Value, args: &ArgMatches) -> Result<()> {
        let mut config = Config::new(args, &Config::full_path(args));
        let mut org_ids = vec![];

        if let Some(organization_memberships) =
            json["client"]["sessions"][0]["user"]["organization_memberships"].as_array()
        {
            for organization in organization_memberships {
                let org_id: String = (organization["id"]).to_string();

                org_ids.push(org_id);
            }
        }

        let user = &json["client"]["sessions"][0]["user"];
        let first_name = &user["first_name"];
        let last_name = &user["last_name"];
        let name = format!("{} {}", &first_name, &last_name);
        let username = (user["username"]).to_string();
        let clerk_id = (user["id"]).to_string();

        let created_at = Utc::now();
        let cloud_account = CloudAccount {
            name: Some(name),
            username: Some(username),
            clerk_id: Some(clerk_id),
            organizations: org_ids, // NOTE: we want to reset/update this with every login
            created_at: Some(created_at),
        };
        config.cloud_account = Some(cloud_account);

        config.write(&Config::full_path(args))?;

        Ok(())
    }
}
