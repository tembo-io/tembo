use crate::cli::context::{list_context, tembo_context_file_path, Context};
use crate::tui::{colors, error};
use anyhow::Error;
use clap::Args;
use colorful::Colorful;
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
    let _ = list_context();
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

    let name = args.name.clone();

    let mut is_set = false;
    for e in data.environment.iter_mut() {
        if e.name == name {
            e.set = Some(true);
            is_set = true
        } else {
            e.set = None
        }
    }

    if !is_set {
        error(&format!(
            "Environment {} not found in Context file ({})",
            name,
            tembo_context_file_path()
        ));
        return Err(Error::msg("Error setting context"));
    }

    if let Err(e) = write_config_to_file(&data, &tembo_context_file_path()) {
        error(&format!("Error: {}", e));
    }

    println!(
        "{} {} {}",
        "✓".color(colors::indicator_good()).bold(),
        colors::gradient_rainbow("Tembo context set to:"),
        name.bold()
    );

    Ok(())
}

fn write_config_to_file(config: &Context, file_path: &str) -> Result<(), anyhow::Error> {
    let toml_string = to_string(config)?;
    let mut file = File::create(file_path)?;

    file.write_all(toml_string.as_bytes())?;

    Ok(())
}
