use super::SubCommand;
use clap::Args;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

// TODO: write to a log rather than printing to stdout

#[derive(Args)]
pub struct InitCommand {
    #[arg(short, long)]
    dry_run: bool,

    #[arg(short, long)]
    file_path: Option<String>,
}

impl SubCommand for InitCommand {
    fn execute(&self) {
        match home::home_dir() {
            Some(_) => {}
            None => println!("Impossible to get your home dir!"), // TODO: catch this and tell user to provide path with a flag
        }

        let path: PathBuf = define_file_path(&self.file_path.clone());

        if self.dry_run {
            println!(
                "config file would be created at: {}",
                &path.to_string_lossy()
            );
        } else {
            println!(
                "config file will be created at: {}",
                &path.to_string_lossy()
            );

            let _ = write_config_file(&path);
        }
    }
}

fn define_file_path(file_path: &Option<String>) -> PathBuf {
    let filename = "conf.toml";

    match file_path {
        Some(path) => PathBuf::from(format!("{}/{}", path, filename)),
        None => {
            let home_dir = home::home_dir().unwrap();

            [home_dir, ".config/tembo".into(), filename.into()]
                .iter()
                .collect()
        }
    }
}

fn write_config_file(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut full_path = path.clone();
    full_path.pop(); // removes the filename and extension

    match create_config_dir(&full_path.to_string_lossy()) {
        Ok(()) => {} // do nothing
        Err(_e) => {
            println!("Directory already exists, moving on");
        }
    }

    let mut file = File::create(&path)?; // create the file
    file.write_all(b"[configuration]")?; // write to the file

    println!("config file written to: {}", &path.to_string_lossy());
    Ok(())
}

fn create_config_dir(dir_path: &str) -> Result<(), Box<dyn Error>> {
    fs::create_dir(dir_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_config_file() {
        assert_eq!(vec!["safe, fast, productive"], search(query, contents));
    }
}
