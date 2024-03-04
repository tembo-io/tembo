use crate::cli::context::{
    get_current_context, tembo_context_file_path, update_access_token, Context, Profile,
};
use crate::tui::error as errors;
use actix_cors::Cors;
use actix_web::{http::header, post, web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Error, Result};
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
struct TokenRequest {
    token: String,
}

pub fn execute() -> Result<(), anyhow::Error> {
    let env = get_current_context()?;
    let profile = env
        .selected_profile
        .as_ref()
        .ok_or_else(|| anyhow!("Cannot log in to the local context, please select a tembo-cloud context before logging in"))?;
    let login_url = url(profile)?;
    let rt = tokio::runtime::Runtime::new().expect("Failed to create a runtime");

    rt.block_on(handle_tokio(login_url))?;

    Ok(())
}

fn url(profile: &Profile) -> Result<String, anyhow::Error> {
    let lifetime = token_lifetime()?;
    let modified_tembo_host = profile.tembo_host.replace("api", "cloud");
    let login_url = modified_tembo_host.clone() + "/cli-success?isCli=true&expiry=" + &lifetime;
    Ok(login_url)
}

async fn handle_tokio(login_url: String) -> Result<(), anyhow::Error> {
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
    body: web::Json<TokenRequest>,
    notify: web::Data<Arc<Notify>>,
) -> impl Responder {
    let token = &body.token;
    let profile_name = read_context();
    if let Err(e) = update_access_token(&profile_name.unwrap(), token) {
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
            .allowed_origin("https://local.tembo.io")
            .allowed_origin("https://cloud.tembo.io")
            .allowed_origin("https://cloud.cdb-dev.com")
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
    println!("Enter the token lifetime in days (1, 7, 30, 365): ");
    io::stdout().flush()?;

    let mut lifetime = String::new();
    io::stdin().read_line(&mut lifetime)?;

    let lifetime = lifetime.trim().to_string();

    Ok(lifetime)
}

fn read_context() -> Result<String, anyhow::Error> {
    let filename = tembo_context_file_path();
    let contents = match fs::read_to_string(&filename) {
        Ok(c) => c,
        Err(e) => {
            errors(&format!("Couldn't read context file {}: {}", filename, e));
            return Err(e.into());
        }
    };
    let mut data: Context = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            errors(&format!("Unable to load data. Error: `{}`", e));
            return Err(e.into());
        }
    };
    for e in data.environment.iter_mut() {
        if e.set == Some(true) {
            if e.name != "local" {
                return Ok(e.name.clone());
            }
        }
    }
    Err(anyhow!("Now "))
}
