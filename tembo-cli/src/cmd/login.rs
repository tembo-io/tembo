use actix_cors::Cors;
use actix_web::{http::header, post, web, App, HttpResponse, HttpServer, Responder};
use anyhow::{Error, Result};
use clap::Args;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{self, Duration};
use webbrowser;

#[derive(Args)]
pub struct LoginCommand {}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

pub async fn execute() -> Result<(), anyhow::Error> {
    let lifetime = token_lifetime()?;
    let login_url = "https://cloud.tembo.io/loginjwt?isCli=true&expiry=".to_owned() + &lifetime;
    webbrowser::open(&login_url)?;

    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    tokio::spawn(async move {
        if let Err(e) = start_server(notify_clone).await {
            eprintln!("Server error: {}", e);
        }
    });

    let result = time::timeout(Duration::from_secs(30), notify.notified()).await;
    match result {
        Ok(_) => println!("File saved and Server Closed!"),
        Err(_) => {
            println!("Operation timed out. Server is being stopped.");
        }
    }

    Ok(())
}

#[post("/")]
async fn handle_request(
    body: web::Json<TokenResponse>,
    notify: web::Data<Arc<Notify>>,
) -> impl Responder {
    let token = &body.token;

    if let Err(e) = save_token_to_file(token) {
        println!("Failed to save token: {}", e);
        return HttpResponse::InternalServerError().body("Failed to save token");
    }

    notify.notify_one();

    HttpResponse::Ok().body(format!("Token received and saved: {}", token))
}

async fn start_server(notify: Arc<Notify>) -> Result<()> {
    let notify_data = web::Data::new(notify.clone());

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("https://pr-462.dkwugv8ig5mdu.amplifyapp.com")
            .allowed_origin("https://local.tembo.io")
            .allowed_origin("https://cloud.tembo.io")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
            .allowed_header(header::CONTENT_TYPE)
            .supports_credentials()
            .max_age(3600);

        App::new()
            .app_data(notify_data.clone())
            .wrap(cors)
            .service(handle_request)
    })
    .bind("127.0.0.1:3000")?
    .run();

    server.await?;

    Ok(())
}

fn token_lifetime() -> Result<String> {
    println!("Enter the token lifetime in days (e.g., 1, 7, 30): ");
    io::stdout().flush()?;

    let mut lifetime = String::new();
    io::stdin().read_line(&mut lifetime)?;

    let lifetime = lifetime.trim().to_string();

    Ok(lifetime)
}

fn save_token_to_file(token: &str) -> Result<(), Error> {
    let home_dir = dirs::home_dir().expect("Could not find home directory");
    let credentials_path = home_dir.join(".tembo/credentials");

    let new_contents = format!(
        "version = \"1.0\"\n\n[[profile]]\nname = 'prod'\ntembo_access_token = \'{}\'\ntembo_host = 'https://api.tembo.io'\ntembo_data_host = 'https://api.data-1.use1.tembo.io'",
        token
    );

    fs::write(&credentials_path, new_contents)?;

    println!("Token updated in credentials file");

    Ok(())
}
