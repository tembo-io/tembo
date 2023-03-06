use super::SubCommand;
use async_trait::async_trait;
use clap::Args;

#[derive(Args)]
pub struct InstallCommand {
    #[arg(long = "pg-config", short = 'p', default_value = "")]
    pg_config: String,
    #[arg(long = "file", short = 'f', default_value = "")]
    file: String,
}

#[async_trait]
impl SubCommand for InstallCommand {
    async fn execute(&self) -> Result<(), anyhow::Error> {
        let mut pg_config = String::new();
        if self.pg_config == "" {
            pg_config = which::which("pg_config")?
                .into_os_string()
                .into_string()
                .unwrap();
        } else {
            // find pg_config in path
            pg_config = self.pg_config.clone();
        }
        println!("Using pg_config: {}", pg_config.clone());
        if self.file == "" {
            println!("trunk only supports installing from a .trunk.tar file, please specify the path to the .trunk.tar file using the --file flag");
            return Ok(());
        }
        // check if self.file is a path that exists to a file
        let path = std::path::Path::new(&self.file);
        if !path.exists() {
            println!("The file {} does not exist", self.file);
            return Ok(());
        }

        let package_lib_dir = std::process::Command::new(pg_config.clone())
            .arg("--pkglibdir")
            .output()
            .expect("failed to execute pg_config")
            .stdout;
        let package_lib_dir = String::from_utf8_lossy(&package_lib_dir)
            .trim_end()
            .to_string();
        let package_lib_dir_path = std::path::PathBuf::from(&package_lib_dir);
        dbg!(&package_lib_dir_path);
        let package_lib_dir = std::fs::canonicalize(&package_lib_dir_path)
            .expect("failed to find path to package lib dir");

        let sharedir = std::process::Command::new(pg_config.clone())
            .arg("--sharedir")
            .output()
            .expect("failed to execute pg_config")
            .stdout;

        let sharedir = String::from_utf8_lossy(&sharedir).trim_end().to_string();
        let sharedir_path = std::path::PathBuf::from(&sharedir).join("extension");
        dbg!(&sharedir_path);
        let sharedir =
            std::fs::canonicalize(sharedir_path).expect("failed to find path to share dir");
        // if this is a symlink, then resolve the symlink

        if !package_lib_dir.exists() && !package_lib_dir.is_dir() {
            println!(
                "The package lib dir {} does not exist",
                package_lib_dir.display()
            );
            return Ok(());
        }
        if !sharedir.exists() && !sharedir.is_dir() {
            println!("The share dir {} does not exist", sharedir.display());
            return Ok(());
        }
        println!("Using pkglibdir: {:?}", package_lib_dir);
        println!("Using sharedir: {:?}", sharedir);
        Ok(())
    }
}
