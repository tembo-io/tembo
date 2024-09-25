use crate::cli::context::{
    get_current_context, list_context, tembo_context_file_path, tembo_credentials_file_path,
    Context, Credential, Environment,
};
use crate::tui::error;
use actix_cors::Cors;
use actix_web::{http::header, post, web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Result};
use clap::Args;
use serde::Deserialize;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Notify;
use tokio::time::{self, Duration};
use webbrowser;

struct SharedState {
    token: Mutex<Option<String>>,
}

#[derive(Args)]
#[clap(author, version, about = "Initiates login sequence to authenticate with Tembo", long_about = None)]
pub struct LoginCommand {
    /// Set your Org ID for your new environment, which starts with "org_"
    #[clap(long)]
    pub organization_id: Option<String>,

    /// Set a name for your new environment, for example "prod". This name will be used for the name of the environment and the credentials profile.
    #[clap(long)]
    pub profile: Option<String>,

    /// Set your tembo_host for your profile, for example api.tembo.io
    #[clap(long)]
    pub tembo_host: Option<String>,

    /// Set your tembo_data_host for your profile, for example api.data-1.use1.tembo.io
    #[clap(long)]
    pub tembo_data_host: Option<String>,
}

#[derive(Deserialize)]
struct TokenRequest {
    token: String,
}

pub fn execute(login_cmd: LoginCommand) -> Result<(), anyhow::Error> {
    let _ = list_context();
    let context_file_path = tembo_context_file_path();
    let contents = fs::read_to_string(context_file_path)?;
    let data: Context = toml::from_str(&contents)?;

    match (&login_cmd.organization_id, &login_cmd.profile) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(anyhow!(
                "Both --organization_id and --profile flags are required when specifying one. Please include values for both flags."
            ));
        }
        (None, None) => {
            let env = get_current_context()?;
            if env.target != "tembo-cloud" {
                return Err(anyhow!(
                    "The local context is currently selected. Please select a context, or initialize a new context with:
            \"tembo login --profile <profile_name> --organization-id <organization_id>\""
                ));
            }
        }
        (Some(_), Some(_)) => {
            if data
                .environment
                .iter()
                .any(|p| &p.name == login_cmd.profile.as_ref().unwrap())
            {
                return Err(anyhow!("An environment with the name {} already exists. Please choose a different name in the --profile flag.", login_cmd.profile.as_ref().unwrap()));
            }
        }
    }

    let login_url = url(login_cmd.tembo_host.as_deref())?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(handle_tokio(login_url, &login_cmd))?;

    Ok(())
}

fn url(cmd: Option<&str>) -> Result<String, anyhow::Error> {
    let lifetime = token_lifetime()?;
    let default_tembo_host = "https://api.tembo.io";
    let modified_tembo_host = cmd.unwrap_or(default_tembo_host);
    let tembo_host = modified_tembo_host.replace("api", "cloud");
    let login_url = tembo_host.clone() + "/cli-success?isCli=true&expiry=" + &lifetime;
    Ok(login_url)
}

async fn handle_tokio(login_url: String, cmd: &LoginCommand) -> Result<(), anyhow::Error> {
    webbrowser::open(&login_url)?;
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();
    let shared_state = web::Data::new(SharedState {
        token: Mutex::new(None),
    });
    let shared_state_clone = shared_state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_server(notify_clone, shared_state).await {
            eprintln!("Server error: {}", e);
        }
    });

    let result = time::timeout(Duration::from_secs(30), notify.notified()).await;
    if let Some(token) = shared_state_clone.token.lock().unwrap().as_ref() {
        let _ = execute_command(cmd, token);
    } else {
        println!("No token was received.");
    }

    match result {
        Ok(_) => {}
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
    shared_state: web::Data<SharedState>,
) -> impl Responder {
    let token = body.token.clone();

    let mut token_storage = shared_state.token.lock().unwrap();
    *token_storage = Some(token.clone());

    notify.notify_one();

    HttpResponse::Ok().body(format!("Token received and saved: {}", token))
}

async fn start_server(notify: Arc<Notify>, shared_state: web::Data<SharedState>) -> Result<()> {
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
            .app_data(shared_state.clone())
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
            error(&format!("Couldn't read context file {}: {}", filename, e));
            return Err(e.into());
        }
    };
    let mut data: Context = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            error(&format!("Unable to load data. Error: `{}`", e));
            return Err(e.into());
        }
    };
    for e in data.environment.iter_mut() {
        if e.set == Some(true) && e.name != "local" {
            return Ok(e.name.clone());
        }
    }
    Err(anyhow!("Cannot read context file "))
}

pub fn update_access_token(
    profile_name: &str,
    new_access_token: &str,
) -> Result<(), anyhow::Error> {
    let credentials_file_path = tembo_credentials_file_path();
    let contents = fs::read_to_string(&credentials_file_path)?;
    let mut credentials: Credential = toml::from_str(&contents)?;

    for profile in &mut credentials.profile {
        if profile.name == profile_name {
            profile.tembo_access_token = new_access_token.to_string();
            break;
        }
    }

    let modified_contents = toml::to_string(&credentials)
        .map_err(|e| anyhow!("Failed to serialize modified credentials: {}", e))?;

    fs::write(&credentials_file_path, modified_contents)
        .map_err(|e| anyhow!("Failed to write modified credentials back to file: {}", e))?;

    Ok(())
}

fn update_context(org_id: &str, profile_name: &str) -> Result<()> {
    let context_file_path = tembo_context_file_path();
    let mut contents = fs::read_to_string(&context_file_path)?;
    let mut data: Context = toml::from_str(&contents)?;

    if let Some(env) = data.environment.iter_mut().find(|p| p.set == Some(true)) {
        env.set = Some(false);
    }

    data.environment.push(Environment {
        name: profile_name.to_string(),
        target: "tembo-cloud".to_string(),
        org_id: Some(org_id.to_string()),
        profile: Some(profile_name.to_string()),
        set: Some(true),
        selected_profile: None,
    });

    contents = toml::to_string(&data)?;
    fs::write(&context_file_path, contents)?;

    Ok(())
}

pub fn update_profile(
    profile_name: &str,
    tembo_host: &str,
    tembo_data_host: &str,
) -> Result<(), anyhow::Error> {
    let credentials_file_path = tembo_credentials_file_path();
    let contents = fs::read_to_string(&credentials_file_path)?;

    if contents.contains(&format!("[[profile]]\nname = \"{}\"", profile_name)) {
        return Ok(());
    }
    let new_profile = format!(
        "\n\n[[profile]]\nname = {:?}\ntembo_access_token = {:?}\ntembo_host = {:?}\ntembo_data_host = {:?}",
        profile_name, "Access token not set yet!", tembo_host, tembo_data_host
    );
    append_to_file(&credentials_file_path, new_profile)?;

    Ok(())
}

fn append_to_file(file_path: &str, content: String) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(file_path)?;
    writeln!(file, "{}", content)?;
    Ok(())
}

pub fn execute_command(cmd: &LoginCommand, token: &str) -> Result<(), anyhow::Error> {
    let default_tembo_host = "https://api.tembo.io";
    let default_tembo_data_host = "https://api.data-1.use1.tembo.io";

    let tembo_host = cmd
        .tembo_host
        .clone()
        .unwrap_or_else(|| default_tembo_host.to_string());
    let tembo_data_host = cmd
        .tembo_data_host
        .clone()
        .unwrap_or_else(|| default_tembo_data_host.to_string());

    match (&cmd.profile, &cmd.organization_id) {
        (Some(profile), Some(org_id)) => {
            update_context(org_id, profile)?;
            update_profile(profile, &tembo_host, &tembo_data_host)?;
            update_access_token(profile, token)?;
        }
        _ => {
            let profile_name = read_context()?;
            update_access_token(&profile_name, token)?;
        }
    }

    Ok(())
}
