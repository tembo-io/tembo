use clap::{Arg, ArgAction, ArgMatches, Command};
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

// Create clap subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("init")
        .about("Initializes a local environment or project; generates configuration")
        .arg(arg!(-d --dryrun "Provides an input file to the program"))
        .arg(
            Arg::new("file-path")
                .short('f')
                .long("file-path")
                .value_name("dir")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .action(ArgAction::Append)
                .help(
                    "A path to the directory to add to the configuration \
                    file to, default is $HOME/.config/tembo",
                ),
        )
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let dry_run: bool = args.get_flag("dryrun");
    let path: PathBuf = define_full_path(args);

    if dry_run {
        println!(
            "config file would be created at: {}",
            &path.to_string_lossy()
        );
    } else {
        println!(
            "config file will be created at: {}",
            &path.to_string_lossy()
        );

        let _ = create_path_and_write_config_file(&path);
    }

    Ok(())
}

fn define_full_path(args: &ArgMatches) -> PathBuf {
    let file_name = "configuration.toml";

    let mut path: PathBuf = get_dir(args);
    path.push(".config");
    path.push("tembo");
    path.push(file_name);

    path
}

fn get_dir(args: &ArgMatches) -> PathBuf {
    // if file-path was provided
    if let Some(p) = args.get_one::<PathBuf>("file-path") {
        if p.is_relative() {
            env::current_dir()
                .expect("Unable to determine the home directory")
                .join(p)
        } else {
            p.to_path_buf()
        }
    } else {
        // if file-path was not provided, use the home directory as a default
        let home_dir = home::home_dir();

        // if home directory can not be determined, use the current directory
        match home_dir {
            Some(path) => path,
            None => env::current_dir().expect("Unable to determine the current directory"),
        }
    }
}

fn create_path_and_write_config_file(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut full_path = path.clone();
    full_path.pop(); // removes any filename and extension

    match create_config_dir(&full_path.to_string_lossy()) {
        Ok(()) => create_config_file(&path.to_string_lossy()),
        Err(e) => {
            println!("Directory can not be created, {}", e);

            return Err(e);
        }
    }
}

fn create_config_dir(dir_path: &str) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir_path)?;
    Ok(())
}

fn create_config_file(path: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::create_new(&path)?; // don't overwrite existing file at path
    file.write_all(b"[configuration]")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn execute_test() {
        // assert that dry-run doesn't write the file
        let file_path = "./test/dryrun/test.toml";
        let path = Path::new(file_path);
        let m = Command::new("myapp")
            .arg(
                Arg::new("dryrun")
                    .value_parser(clap::value_parser!(bool))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            );

        let _ = execute(&m.get_matches_from(vec!["myapp", "true", &file_path]));
        assert_eq!(path.exists(), false);
    }

    #[test]
    fn get_dir_test() {
        // with a file-path
        let file_path = "/foo";
        let m = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp", &file_path]);

        assert_eq!(get_dir(&m).to_str(), Some(file_path));

        // without a file-path
        let m = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        assert_eq!(get_dir(&m).to_str(), home::home_dir().unwrap().to_str());
    }

    #[test]
    fn create_path_and_write_config_file_test() {
        let mut path: PathBuf = env::current_dir().unwrap();
        path.push("tests");
        path.push(".config");
        path.push("tembo");
        path.push("configuration.toml");

        let write = create_path_and_write_config_file(&path);
        assert_eq!(write.is_ok(), true);

        let overwrite = create_path_and_write_config_file(&path);
        assert_eq!(overwrite.is_err(), true);

        let _file = std::fs::remove_file(&*path.to_string_lossy());
        let _dir = std::fs::remove_dir(&*path.to_string_lossy());
    }

    #[test]
    fn create_config_dir_test() {
        let mut path: PathBuf = env::current_dir().unwrap();
        path.push("tests");
        path.push(".config");

        let write = create_config_dir(&path.to_string_lossy());
        assert_eq!(write.is_ok(), true);

        let overwrite = create_config_file(&path.to_string_lossy());
        assert_eq!(overwrite.is_err(), true);

        // clean up
        let _ = std::fs::remove_dir(&*path.to_string_lossy());
    }

    #[test]
    fn create_config_file_test() {
        let mut path: PathBuf = env::current_dir().unwrap();
        path.push("tests");
        path.push("configuration.toml");

        let write = create_config_file(&path.to_string_lossy());
        assert_eq!(write.is_ok(), true);

        let overwrite = create_config_file(&path.to_string_lossy());
        assert_eq!(overwrite.is_err(), true);

        // clean up
        let _ = std::fs::remove_file(&*path.to_string_lossy());
    }
}
