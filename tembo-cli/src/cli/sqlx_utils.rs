use sqlx::postgres::PgConnectOptions;

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
