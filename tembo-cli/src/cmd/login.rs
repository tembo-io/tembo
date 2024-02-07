use hyper::{Body, Request, Response, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use std::fs;
use std::sync::Arc;
use clap::Args;
use tokio::sync::Notify;
use serde::Deserialize;
use webbrowser;
use anyhow::Error;
use std::io::{self, Write};

#[derive(Args)]
pub struct LoginCommand {}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

pub async fn execute() -> Result<(), anyhow::Error> {
    let expiry_days = prompt_for_token_expiry();
    let login_url = "https://local.tembo.io/loginjwt?isCli=true";
    webbrowser::open(&login_url)?;

    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    tokio::spawn(async move {
        if let Err(e) = start_server(notify_clone).await {
            eprintln!("Server error: {}", e);
        }
    });
    
    // Wait for the notify to signal shutdown
    notify.notified().await;
    send_expiry_to_cloud();

    println!("Token saved. Exiting the program.");

    Ok(())
}

fn prompt_for_token_expiry() -> u32 {
    println!("Expiry in days for the token (1, 7, 30, 365): ");
    let mut expiry_days = String::new();
    io::stdout().flush().unwrap(); // Make sure the prompt is displayed before reading input
    io::stdin().read_line(&mut expiry_days).expect("Failed to read line");
    let expiry_days: u32 = expiry_days.trim().parse().unwrap_or(1); // Default to 1 if parsing fails
    expiry_days
}

fn send_expiry_to_cloud() -> Result<(), Error> {
    let client = reqwest::Client::new();
    let res = client.post("https://local.tembo.io/api/token")
    .body("the exact body that is sent")
    .send()
    ;

    print!("ok");

    Ok(())
}


async fn handle_request(req: Request<Body>, notify: Arc<Notify>) -> Result<Response<Body>, hyper::Error> {
    if req.method() == hyper::Method::POST {
        let whole_body = match hyper::body::to_bytes(req.into_body()).await {
            Ok(body) => body,
            Err(_) => return Ok(Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from("Failed to read request body")).unwrap()),
        };
        
        let body_str = match String::from_utf8(whole_body.to_vec()) {
            Ok(s) => s,
            Err(_) => return Ok(Response::builder().status(StatusCode::BAD_REQUEST).body(Body::from("Request body decode error")).unwrap()),
        };

        // Parse the JSON to get the token
        let token_response: TokenResponse = match serde_json::from_str(&body_str) {
            Ok(parsed) => parsed,
            Err(_) => return Ok(Response::builder().status(StatusCode::BAD_REQUEST).body(Body::from("Failed to parse JSON")).unwrap()),
        };

        // Use the extracted token
        match save_token_to_file(&token_response.token) {
            Ok(_) => notify.notify_one(),
            Err(_) => return Ok(Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from("Failed to save token")).unwrap()),
        }

        Ok(Response::new(Body::from("Token received and processed")))
    } else {
        Ok(Response::builder().status(StatusCode::METHOD_NOT_ALLOWED).body(Body::from("Method not allowed")).unwrap())
    }
}


async fn start_server(notify: Arc<Notify>) -> Result<(), anyhow::Error> {
    let addr = ([127, 0, 0, 1], 3000).into();
    let server = Server::bind(&addr).serve(make_service_fn(move |_conn| {
        let notify_clone = notify.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| handle_request(req, notify_clone.clone())))
        }
    }));

    println!("Server running on http://localhost:3000");

    // Await the server completion, including graceful shutdown
    server.await.map_err(anyhow::Error::from)
}

fn save_token_to_file(token: &str) -> Result<(), Error> {
    let home_dir = dirs::home_dir().expect("Could not find home directory");
    let credentials_path = home_dir.join(".tembo/credentials");

    let new_contents = format!(
        "version = \"1.0\"\n\n[[profile]]\nname = 'prod'\ntembo_access_token = \"{}\"\ntembo_host = 'https://api.tembo.io'\ntembo_data_host = 'https://api.data-1.use1.tembo.io'",
        token
    );

    fs::write(&credentials_path, new_contents)?;

    println!("Token updated in credentials file");

    Ok(())
}