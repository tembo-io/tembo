use crate::tui::colors;
use spinners::Spinner;
use spinners::Spinners;
use sqlx::migrate::Migrator;
use sqlx::Pool;
use sqlx::Postgres;
use std::path::Path;
use temboclient::models::ConnectionInfo;
use colorful::{Color, Colorful};

pub struct SqlxUtils {}

impl SqlxUtils {
    // run sqlx migrate
    pub async fn run_migrations(connection_info: ConnectionInfo) -> Result<(), anyhow::Error> {
        let mut sp = Spinner::new(Spinners::Dots, "Running SQL migration".into());

        let connection_string = format!(
            "postgresql://{}:{}@{}:{}",
            connection_info.user,
            connection_info.password,
            connection_info.host,
            connection_info.port
        );

        let pool = Pool::<Postgres>::connect(connection_string.as_str()).await?;

        let m = Migrator::new(Path::new("./migrations")).await?;
        m.run(&pool).await?;

        sp.stop_with_message(format!(
            "{} {}",
            "✓".color(colors::indicator_good()).bold(),
            "SQL migration completed".color(Color::White).bold()
        ));

        Ok(())
    }
}
