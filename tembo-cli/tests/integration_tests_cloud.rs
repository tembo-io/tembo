use assert_cmd::Command;
use random_string::generate;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use tembo::cli::context::{
    get_current_context, tembo_context_file_path, tembo_credentials_file_path, Environment,
    CONTEXT_EXAMPLE_TEXT, CREDENTIALS_EXAMPLE_TEXT,
};
use temboclient::apis::configuration::Configuration;
use temboclient::apis::instance_api::get_all;
use temboclient::models::{Instance, State};

const CARGO_BIN: &str = "tembo";

#[tokio::test]
async fn minimal_cloud() -> Result<(), Box<dyn Error>> {
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir).join("examples").join("minimal");

    env::set_current_dir(&test_dir)?;

    replace_file(tembo_context_file_path(), CONTEXT_EXAMPLE_TEXT)?;
    replace_file(tembo_credentials_file_path(), CREDENTIALS_EXAMPLE_TEXT)?;

    // tembo init
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("init");
    cmd.assert().success();

    let charset = "abcdefghijklmnopqrstuvwxyz";
    let instance_name = format!("e2e-cli-{}", generate(10, charset));

    setup_env(&instance_name)?;

    // tembo context set --name prod
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("context");
    cmd.arg("set");
    cmd.arg("--name");
    cmd.arg("prod");
    cmd.assert().success();

    // tembo apply
    let mut cmd = Command::cargo_bin(CARGO_BIN).unwrap();
    cmd.arg("apply");
    cmd.assert().success();

    let output = cmd.output()?;

    if output
        .stdout
        .windows(b"Error creating instance".len())
        .any(|window| window == b"Error creating instance")
    {
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));
        panic!("Error: Instance creation failed");
    }

    let env = get_current_context()?;
    let profile = env.clone().selected_profile.unwrap();
    let config = Configuration {
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(profile.tembo_access_token),
        ..Default::default()
    };

    for attempt in 1..=5 {
        let maybe_instance = get_instance(&instance_name, &config, &env).await?;
        if let Some(instance) = maybe_instance {
            println!("Instance is {:?}", instance.state);
            if instance.state == State::Up {
                break;
            }

            if attempt == 5 {
                assert_eq!(instance.state, State::Up, "Instance isn't Up")
            }
        } else if attempt == 5 {
            panic!("Failed to create instance");
        }

        // Wait a bit until trying again
        tokio::time::sleep(Duration::from_secs(30)).await;
    }

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    tokio::time::sleep(Duration::from_secs(10)).await;

    let maybe_instance = get_instance(&instance_name, &config, &env).await?;
    if let Some(instance) = maybe_instance {
        assert_eq!(instance.state, State::Deleting, "Instance isn't Deleting")
    } else {
        assert!(true, "Instance isn't Deleting")
    }

    replace_vars_in_file(
        "tembo.toml".to_string(),
        &format!("instance_name = \"{instance_name}\""),
        "instance_name = \"minimal\"",
    )?;

    Ok(())
}

fn setup_env(instance_name: &String) -> Result<(), Box<dyn Error>> {
    replace_vars_in_file(tembo_context_file_path(), "ORG_ID", &env::var("ORG_ID")?)?;

    replace_vars_in_file(
        tembo_credentials_file_path(),
        "ACCESS_TOKEN",
        &env::var("ACCESS_TOKEN")?,
    )?;

    replace_vars_in_file(
        tembo_credentials_file_path(),
        "https://api.tembo.io",
        &env::var("TEMBO_HOST")?,
    )?;

    replace_vars_in_file(
        tembo_credentials_file_path(),
        "https://api.data-1.use1.tembo.io",
        &env::var("TEMBO_DATA_HOST")?,
    )?;

    replace_vars_in_file(
        "tembo.toml".to_string(),
        "instance_name = \"minimal\"",
        &format!("instance_name = \"{instance_name}\""),
    )?;

    Ok(())
}

fn replace_vars_in_file(
    file_path: String,
    word_from: &str,
    word_to: &str,
) -> Result<(), Box<dyn Error>> {
    let mut src = File::open(&file_path)?;
    let mut data = String::new();
    src.read_to_string(&mut data)?;
    drop(src);
    let new_data = data.replace(word_from, word_to);
    let mut dst = File::create(&file_path)?;
    dst.write(new_data.as_bytes())?;
    drop(dst);

    Ok(())
}

fn replace_file(file_path: String, word_to: &str) -> Result<(), Box<dyn Error>> {
    let mut dst = File::create(&file_path)?;
    dst.write_all(word_to.as_bytes())?;
    drop(dst);
    Ok(())
}

pub async fn get_instance(
    instance_name: &str,
    config: &Configuration,
    env: &Environment,
) -> Result<Option<Instance>, anyhow::Error> {
    let v = get_all(config, env.org_id.clone().unwrap().as_str()).await;

    println!("OrgID: {}", env.org_id.clone().unwrap().as_str());

    match v {
        Ok(result) => {
            let maybe_instance = result
                .iter()
                .find(|instance| instance.instance_name == instance_name);

            if let Some(instance) = maybe_instance {
                return Ok(Some(instance.clone()));
            }
        }
        Err(error) => eprintln!("Error getting instance: {}", error),
    };
    Ok(None)
}
