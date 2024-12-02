use clap::Args;
use reqwest::blocking::Client;
use std::fs::{self, Permissions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Updates Tembo-CLI to the latest version
#[derive(Args)]
pub struct UpdateCommand {}

pub fn execute(_verbose: bool) -> Result<(), anyhow::Error> {
    let (latest_version, release_date) = fetch_latest_release()?;

    let asset_url = get_asset_url(&latest_version, &release_date)?;
    println!("Downloading the latest version from: {}", asset_url);

    let binary_path = download_binary(&asset_url)?;

    let current_binary_path = std::env::current_exe()?;
    replace_binary(&binary_path, &current_binary_path)?;

    println!(
        "Successfully updated Tembo CLI to version {} (Released on {}).",
        latest_version, release_date
    );

    Ok(())
}

fn fetch_latest_release() -> Result<(String, String), anyhow::Error> {
    let url = "https://api.github.com/repos/tembo-io/tembo/releases";
    let client = Client::new();

    let response: serde_json::Value = client
        .get(url)
        .header("User-Agent", "tembo-cli")
        .send()?
        .json()?;

    let releases = response
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Failed to fetch releases from the GitHub API"))?;

    let latest_release = releases
        .iter()
        .max_by_key(|release| {
            release["published_at"]
                .as_str()
                .map(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .flatten()
        })
        .ok_or_else(|| anyhow::anyhow!("No releases found in the GitHub API response"))?;

    let release_date = latest_release["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to fetch the release tag"))?
        .to_string();

    let assets = latest_release["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Failed to fetch assets for the latest release"))?;

    let version = assets
        .iter()
        .filter_map(|asset| asset["name"].as_str())
        .find(|name| name.contains("tembo-cli-") && name.ends_with(".tar.gz"))
        .and_then(|name| {
            let parts: Vec<&str> = name.split('-').collect();
            if parts.len() > 2 {
                Some(parts[2].to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to determine the CLI version from assets"))?;

    println!(
        "Latest release: {}, Version: {}, Published at: {}",
        release_date, version, latest_release["published_at"]
    );

    Ok((version, release_date))
}

fn get_asset_url(version: &str, release_date: &str) -> Result<String, anyhow::Error> {
    let platform = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            format!("tembo-cli-{}-aarch64-apple.tar.gz", version)
        } else {
            format!("tembo-cli-{}-x86_64-apple.tar.gz", version)
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            format!("tembo-cli-{}-aarch64-linux.tar.gz", version)
        } else {
            format!("tembo-cli-{}-x86_64-linux.tar.gz", version)
        }
    } else if cfg!(target_os = "windows") {
        format!("tembo-{}-x86_64-windows.tar.gz", version)
    } else {
        return Err(anyhow::anyhow!("Unsupported operating system"));
    };

    let asset_url = format!(
        "https://github.com/tembo-io/tembo/releases/download/{}/{}",
        release_date, platform
    );

    Ok(asset_url)
}

fn download_binary(url: &str) -> Result<String, anyhow::Error> {
    let response = reqwest::blocking::get(url)?;

    let tmp_file = std::env::temp_dir().join("tembo-cli-update.tar.gz");
    let mut file = fs::File::create(&tmp_file)?;
    file.write_all(&response.bytes()?)?;

    let extract_dir = std::env::temp_dir().join("tembo-cli-update");
    fs::create_dir_all(&extract_dir)?;

    let output = Command::new("tar")
        .args(["-xzf", tmp_file.to_str().unwrap(), "-C", extract_dir.to_str().unwrap()])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to extract the binary: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let extracted_binary = extract_dir.join("tembo");

    Ok(extracted_binary.to_str().unwrap().to_string())
}

fn replace_binary(new_binary_path: &str, current_binary_path: &Path) -> Result<(), anyhow::Error> {
    let backup_path = current_binary_path.with_extension("bak");
    fs::rename(current_binary_path, &backup_path)?;

    fs::copy(new_binary_path, current_binary_path)?;
    fs::set_permissions(current_binary_path, Permissions::from_mode(0o755))?;

    Ok(())
}
