use crate::tui::colors;
use colorful::{Color, Colorful};
use sqlx::migrate::Migrator;
use sqlx::Pool;
use sqlx::Postgres;
use std::path::Path;
use temboclient::models::ConnectionInfo;
use spinoff::{spinners, Spinner};

pub struct SqlxUtils {}

impl SqlxUtils {
    // run sqlx migrate
    pub async fn run_migrations(connection_info: ConnectionInfo) -> Result<(), anyhow::Error> {
        let mut sp = Spinner::new(spinners::Dots, "Running SQL migration", spinoff::Color::White);

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

        sp.stop_with_message(&format!(
            "{} {}",
            "✓".color(colors::indicator_good()).bold(),
            "SQL migration completed".color(Color::White).bold()
        ));

        Ok(())
    }
}
