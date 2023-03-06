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
        // execute pg_config to get the environment
        let pg_config_output = std::process::Command::new(pg_config.clone())
            .output()
            .expect("failed to execute pg_config");
        println!(
            "pg_config output: {}",
            String::from_utf8_lossy(&pg_config_output.stdout)
        );
        // RUN cp -r ${BUILD_DIR}$(/usr/bin/pg_config --pkglibdir)/*.so $OUTPUT_DIR && \
        // cp -r ${BUILD_DIR}$(/usr/bin/pg_config --sharedir)/extension/* $OUTPUT_DIR
        let package_lib_dir = std::process::Command::new(pg_config.clone())
            .arg("--pkglibdir")
            .output()
            .expect("failed to execute pg_config")
            .stdout;
        let package_lib_dir = String::from_utf8_lossy(&package_lib_dir).to_string();
        let package_lib_dir = std::path::Path::new(&package_lib_dir);

        let sharedir = std::process::Command::new(pg_config.clone())
            .arg("--sharedir")
            .output()
            .expect("failed to execute pg_config")
            .stdout;

        let sharedir = String::from_utf8_lossy(&sharedir).to_string();
        let sharedir = std::path::Path::new(&sharedir);

        println!("Using pklibdir: {:?}", package_lib_dir);
        println!("Using pg_config: {:?}", sharedir);
        Ok(())
    }
}
