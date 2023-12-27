use curl::easy::Easy;
use simplelog::*;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub struct FileUtils {}

impl FileUtils {
    pub fn create_dir(dir_name: String, dir_path: String) -> Result<(), anyhow::Error> {
        if Path::new(&dir_path).exists() {
            info!("Tembo {} path exists", dir_name);
            return Ok(());
        }

        match fs::create_dir_all(dir_path) {
            Err(why) => panic!("Couldn't create {}: {}", dir_name, why),
            Ok(_) => info!("Tembo {} created", dir_name),
        };

        Ok(())
    }

    pub fn create_file(
        file_name: String,
        file_path: String,
        file_content: String,
        recreate: bool,
    ) -> Result<(), anyhow::Error> {
        let path = Path::new(&file_path);
        if !recreate && path.exists() {
            info!("Tembo {} file exists", file_name);
            return Ok(());
        }
        let display = path.display();
        let mut file: File = match File::create(path) {
            Err(why) => panic!("Couldn't create {}: {}", display, why),
            Ok(file) => file,
        };
        info!("Tembo {} file created", file_name);

        if let Err(e) = file.write_all(file_content.as_bytes()) {
            panic!("Couldn't write to context file: {}", e);
        }
        Ok(())
    }

    pub fn download_file(
        file_path: &str,
        download_location: &str,
        recreate: bool,
    ) -> std::io::Result<()> {
        let path = Path::new(&download_location);
        if !recreate && path.exists() {
            info!("Tembo {} file exists", download_location);
            return Ok(());
        }

        let mut dst = Vec::new();
        let mut easy = Easy::new();
        easy.url(file_path).unwrap();
        let _redirect = easy.follow_location(true);

        {
            let mut transfer = easy.transfer();
            transfer
                .write_function(|data| {
                    dst.extend_from_slice(data);
                    Ok(data.len())
                })
                .unwrap();
            transfer.perform().unwrap();
        }
        {
            let mut file = File::create(download_location)?;
            file.write_all(dst.as_slice())?;
        }
        Ok(())
    }

    pub fn get_current_working_dir() -> String {
        env::current_dir().unwrap().to_str().unwrap().to_string()
    }
}
