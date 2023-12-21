use crate::Result;
use base64::{engine::general_purpose, Engine as _};
use spinners::Spinner;
use spinners::Spinners;
use sqlx::migrate::Migrator;
use sqlx::Pool;
use sqlx::Postgres;
use std::path::Path;
use temboclient::models::ConnectionInfo;

pub struct SqlxUtils {}

impl SqlxUtils {
    // run sqlx migrate
    pub async fn run_migrations(connection_info: ConnectionInfo, decode: bool) -> Result {
        let mut sp = Spinner::new(Spinners::Line, "Running SQL migration".into());

        let user: String;
        let pwd: String;

        if decode {
            user = SqlxUtils::b64_decode(&connection_info.user);
            pwd = SqlxUtils::b64_decode(&connection_info.password);
        } else {
            user = connection_info.user;
            pwd = connection_info.password;
        }

        let connection_string = format!(
            "postgresql://{}:{}@{}:{}",
            user, pwd, connection_info.host, connection_info.port
        );

        let pool = Pool::<Postgres>::connect(connection_string.as_str()).await?;

        let m = Migrator::new(Path::new("./migrations")).await?;
        m.run(&pool).await?;

        sp.stop_with_message("- SQL migration completed".to_string());

        Ok(())
    }

    fn b64_decode(b64_encoded: &str) -> String {
        let bytes = general_purpose::STANDARD.decode(b64_encoded).unwrap();
        String::from_utf8(bytes).unwrap()
    }
}
