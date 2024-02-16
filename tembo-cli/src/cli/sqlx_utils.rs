use crate::tui::colors;
use colorful::{Color, Colorful};
use spinoff::{spinners, Spinner};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnectOptions;
use sqlx::Pool;
use sqlx::Postgres;
use std::error::Error;
use std::path::Path;
use temboclient::models::ConnectionInfo;

pub struct SqlxUtils {}

impl SqlxUtils {
    pub async fn execute_sql(instance_name: String, sql: String) -> Result<(), Box<dyn Error>> {
        // Configure SQLx connection options
        let connect_options = PgConnectOptions::new()
            .username("postgres")
            .password("postgres")
            .host(&format!("{}.local.tembo.io", instance_name))
            .database("postgres");

        // Connect to the database
        let pool = sqlx::PgPool::connect_with(connect_options).await?;

        // Simple query
        sqlx::query(&sql).fetch_optional(&pool).await?;

        println!(
            "Successfully connected to the database: {}",
            &format!("{}.local.tembo.io", instance_name)
        );

        Ok(())
    }

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

        let path = instance_name.clone() + "/migrations";
        let m = Migrator::new(Path::new(&path)).await?;
        m.run(&pool).await?;

        sp.stop_with_message(&format!(
            "{} {}",
            "✓".color(colors::indicator_good()).bold(),
            format!("SQL migration completed for {}", instance_name)
                .color(Color::White)
                .bold()
        ));

        Ok(())
    }
}
