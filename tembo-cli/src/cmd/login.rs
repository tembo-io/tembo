use crate::cli::context::Environment;
use crate::cli::context::{
    get_current_context, tembo_context_file_path, tembo_credentials_file_path, Context, Credential,
    Profile,
};
use crate::tui::error;
use actix_cors::Cors;
use actix_web::{http::header, post, web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Result};
use clap::Args;
use serde::Deserialize;
use std::fs;
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
#[clap(author, version, about, long_about = None)]
pub struct LoginCommand {
    #[clap(long)]
    pub organization_id: Option<String>,

    #[clap(long)]
    pub profile: Option<String>,

    #[clap(long)]
    pub tembo_host: Option<String>,

    #[clap(long)]
    pub tembo_data_host: Option<String>,
}

#[derive(Deserialize)]
struct TokenRequest {
    token: String,
}

pub fn execute(login_cmd: LoginCommand) -> Result<(), anyhow::Error> {
    let env = get_current_context()?;

    match (&login_cmd.organization_id, &login_cmd.profile) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(anyhow!(
                "Both 'organization_id' and 'profile' must be specified."
            ));
        }
        (None, None) => {}
        _ => {}
    }

    if env.target == "tembo-cloud" {
        let profile = env
            .selected_profile
            .as_ref()
            .ok_or_else(|| anyhow!("Environment not setup properly"))?;
        let login_url = url(profile)?;
        let rt = tokio::runtime::Runtime::new().expect("Failed to create a runtime");
        rt.block_on(handle_tokio(login_url, login_cmd))?;
    } else {
        print!("Cannot log in to the local context. Please select a context, or initialize a new context with tembo login --profile < name your profile > --organization-id < Your Tembo Cloud organization ID >");
    }

    Ok(())
}

fn url(profile: &Profile) -> Result<String, anyhow::Error> {
    let lifetime = token_lifetime()?;
    let modified_tembo_host = profile.get_tembo_host().replace("api", "cloud");
    let login_url = modified_tembo_host.clone() + "/cli-success?isCli=true&expiry=" + &lifetime;
    Ok(login_url)
}

async fn handle_tokio(login_url: String, cmd: LoginCommand) -> Result<(), anyhow::Error> {
    webbrowser::open(&login_url)?;
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();
    let profile_name = read_context();
    let shared_state = web::Data::new(SharedState {
        token: Mutex::new(None),
    });
    let shared_state_clone = shared_state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_server(notify_clone, shared_state_clone).await {
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
    if let Some(token) = shared_state.token.lock().unwrap().as_ref() {
        let _ = execute_command(cmd, token, &profile_name.unwrap());
    } else {
        println!("No token was received.");
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
    Err(anyhow!("Now "))
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

pub fn update_context(org_id: &str, profile_name: &str) -> Result<(), anyhow::Error> {
    let context_file_path = tembo_context_file_path();
    let contents = fs::read_to_string(&context_file_path)?;
    let mut data: Context = toml::from_str(&contents)?;

    let new_env = Environment {
        name: profile_name.to_string(),
        target: "tembo-cloud".to_string(),
        org_id: Some(org_id.to_string()),
        profile: Some(profile_name.to_string()),
        set: Some(false),
        selected_profile: None,
    };
    data.environment.push(new_env);

    let modified_contents = toml::to_string(&data)?;
    fs::write(&context_file_path, modified_contents)?;

    Ok(())
}

pub fn update_or_create_profile(
    profile_name: &str,
    new_access_token: &str,
    tembo_host: &str,
    tembo_data_host: &str,
) -> Result<(), anyhow::Error> {
    let credentials_file_path = tembo_credentials_file_path();
    let contents = fs::read_to_string(&credentials_file_path)?;
    let mut credentials: Credential = toml::from_str(&contents)?;

    let profile_opt = credentials
        .profile
        .iter_mut()
        .find(|p| p.name == profile_name);

    match profile_opt {
        Some(profile) => {
            profile.tembo_access_token = new_access_token.to_string();
        }
        None => {
            let new_profile = Profile {
                name: profile_name.to_string(),
                tembo_access_token: new_access_token.to_string(),
                tembo_host: tembo_host.to_string(),
                tembo_data_host: tembo_data_host.to_string(),
            };
            credentials.profile.push(new_profile);
        }
    }

    let modified_contents = toml::to_string(&credentials)?;
    fs::write(&credentials_file_path, modified_contents)?;

    Ok(())
}

pub fn execute_command(
    cmd: LoginCommand,
    new_access_token: &str,
    profile_name: &str,
) -> Result<(), anyhow::Error> {
    let profile = cmd.profile;
    let org_id = cmd.organization_id;
    let default_tembo_host = "https://api.tembo.io";
    let default_tembo_data_host = "https://api.data-1.use1.tembo.io";

    let tembo_host = cmd
        .tembo_host
        .unwrap_or_else(|| default_tembo_host.to_string());
    let tembo_data_host = cmd
        .tembo_data_host
        .unwrap_or_else(|| default_tembo_data_host.to_string());

    if profile.is_some() && org_id.is_some() {
        update_or_create_profile(
            &profile.clone().unwrap(),
            new_access_token,
            &tembo_host.clone(),
            &tembo_data_host.clone(),
        )?;
        update_context(&org_id.unwrap(), &profile.clone().unwrap())?;
    } else {
        let _ = update_access_token(profile_name, new_access_token);
    }

    Ok(())
}
