use crate::cli::context::{tembo_context_file_path, Context};
use clap::Args;
use std::fs::{self, File};
use std::io::Write;
use toml::to_string;

// Arguments for 'context set'
#[derive(Args)]
pub struct ContextSetArgs {
    #[clap(short, long)]
    pub name: String,
}

pub fn execute(args: &ContextSetArgs) -> Result<(), anyhow::Error> {
    let filename = tembo_context_file_path();

    let contents = match fs::read_to_string(&filename) {
        Ok(c) => c,
        Err(e) => {
            panic!("Couldn't read context file {}: {}", filename, e);
        }
    };

    let mut data: Context = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            panic!("Unable to load data. Error: `{}`", e);
        }
    };

    let name = args.name.clone();

    for e in data.environment.iter_mut() {
        if e.name == name {
            e.set = Some(true)
        } else {
            e.set = None
        }
    }

    if let Err(e) = write_config_to_file(&data, &tembo_context_file_path()) {
        eprintln!("Error: {}", e);
    }

    println!("Tembo context set to: {}", name);

    Ok(())
}

fn write_config_to_file(config: &Context, file_path: &str) -> Result<(), anyhow::Error> {
    let toml_string = to_string(config)?;
    let mut file = File::create(file_path)?;

    file.write_all(toml_string.as_bytes())?;

    Ok(())
}
