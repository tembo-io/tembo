use crate::tui::colors;
use colorful::{Color, Colorful};
use spinoff::{spinners, Spinner};
use sqlx::migrate::Migrator;
use sqlx::Pool;
use sqlx::Postgres;
use std::path::Path;
use temboclient::models::ConnectionInfo;

pub struct SqlxUtils {}

impl SqlxUtils {
    // run sqlx migrate
    pub async fn run_migrations(
        connection_info: ConnectionInfo,
        instance_name: String,
    ) -> Result<(), anyhow::Error> {
        let mut sp = Spinner::new(
            spinners::Dots,
            "Running SQL migration",
            spinoff::Color::White,
        );

        let connection_string = format!(
            "postgresql://{}:{}@{}:{}",
            connection_info.user,
            connection_info.password,
            connection_info.host,
            connection_info.port
        );

        let pool = Pool::<Postgres>::connect(connection_string.as_str()).await?;

        let path = instance_name + "/migrations";
        let m = Migrator::new(Path::new(&path)).await?;
        m.run(&pool).await?;

        sp.stop_with_message(&format!(
            "{} {}",
            "✓".color(colors::indicator_good()).bold(),
            "SQL migration completed".color(Color::White).bold()
        ));

        Ok(())
    }
}
