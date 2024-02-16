use crate::tui::colors;
use colorful::{Color, Colorful};
use spinoff::{spinners, Spinner};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnectOptions;
use sqlx::Pool;
use sqlx::Postgres;
use std::path::Path;
use temboclient::models::ConnectionInfo;

pub struct SqlxUtils {}

impl SqlxUtils {
    pub async fn execute_sql(instance_name: String, sql: String) -> Result<(), anyhow::Error> {
        let connect_options = PgConnectOptions::new()
            .username("postgres")
            .password("postgres")
            .host(&format!("{}.local.tembo.io", instance_name))
            .database("postgres");

        let pool = sqlx::PgPool::connect_with(connect_options).await?;

        sqlx::query(&sql).execute(&pool).await?;

        Ok(())
    }
}
