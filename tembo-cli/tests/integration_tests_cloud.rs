use assert_cmd::Command;
use random_string::generate;
use std::env;
use std::error::Error;
use std::fs::File;
use regex::Regex;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use tembo::cli::context::{
    get_current_context, tembo_context_file_path, tembo_credentials_file_path, Environment,
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

    // tembo init
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("init");
    cmd.assert().success();

    let charset = "abcdefghijklmnopqrstuvwxyz";
    let instance_name = generate(10, charset);

    setup_env(&instance_name)?;

    // tembo context set --name local
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("context");
    cmd.arg("set");
    cmd.arg("--name");
    cmd.arg("prod");
    cmd.assert().success();


    let env = get_current_context()?;
    println!("{:?}",env);
    let profile = env.clone().selected_profile.unwrap();
    let config = Configuration {
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(profile.tembo_access_token),
        ..Default::default()
    };

    // tembo apply
    let mut cmd = Command::cargo_bin(CARGO_BIN).unwrap();
    cmd.arg("apply");
    cmd.assert().success();


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
        "tembo.toml".to_string().into(),
        &format!("instance_name = \"{instance_name}\""),
        "instance_name = \"minimal\"",
    )?;

    Ok(())
}

fn setup_env(instance_name: &String) -> Result<(), Box<dyn Error>> {
    let context_template = "# [[environment]]
# name = 'prod'
# target = 'tembo-cloud'
# org_id can be found in your tembo cloud url. Example: org_2bVDi36rsJNot2gwzP37enwxzMk
# org_id = 'Org ID here'
# profile = 'prod'";

    let context_replacement = format!(
        "[[environment]]
name = 'prod'
target = 'tembo-cloud'
org_id = '{}'
profile = 'prod'",
        env::var("ORG_ID")?
    );

    replace_vars_in_file(
        tembo_context_file_path().into(),
        context_template,
        &context_replacement,
    )?;

    let profile_template = "# [[profile]]
# name = 'prod'
# Generate an Access Token either through 'tembo login' or visit 'https://cloud.tembo.io/generate-jwt'
# tembo_access_token = 'Access token here'
# tembo_host = 'https://api.tembo.io'
# tembo_data_host = 'https://api.data-1.use1.tembo.io'";

    let profile_replacement = format!(
        "[[profile]]
name = 'prod'
tembo_access_token = '{}'
tembo_host = '{}'
tembo_data_host = '{}'",
        env::var("ACCESS_TOKEN")?,
        env::var("TEMBO_HOST")?,
        env::var("TEMBO_DATA_HOST")?
    );

    replace_vars_in_file(
        tembo_credentials_file_path().into(),
        profile_template,
        &profile_replacement,
    )?;

    replace_vars_in_file(
        "tembo.toml".to_string().into(),
        "instance_name = \"minimal\"",
        &format!("instance_name = \"{instance_name}\""),
    )?;

    Ok(())
}

fn replace_vars_in_file(
    file_path: PathBuf,
    pattern: &str,
    replacement: &str,
) -> Result<(), Box<dyn Error>> {
    let mut file_contents = {
        let mut fc = String::new();
        File::open(&file_path)?.read_to_string(&mut fc)?;
        fc
    };

    let re = Regex::new(pattern)?;
    let new_contents = re.replace_all(&file_contents, replacement).to_string();

    File::create(&file_path)?.write_all(new_contents.as_bytes())?;

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
