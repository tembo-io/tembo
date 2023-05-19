use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tracing::debug;

use crate::{apis::coredb_types::CoreDB, defaults, Context, Error};


#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
#[allow(non_snake_case)]
pub struct PostgresMetrics {
    #[serde(default = "defaults::default_image")]
    pub image: String,
    #[serde(default = "defaults::default_postgres_exporter_enabled")]
    pub ExporterEnabled: bool,

    #[serde(flatten)]
    pub queries: Option<HashMap<String, Metric>>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub usage: Usage,
    pub description: String,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    #[serde(flatten)]
    pub metrics: HashMap<String, Metric>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct QueryItem {
    pub query: String,
    pub master: bool,
    pub metrics: Vec<Metrics>,
}

use std::str::FromStr;

#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize, PartialEq)]
pub enum Usage {
    Counter,
    Gauge,
    Histogram,
    Label,
}

impl FromStr for Usage {
    type Err = ();

    fn from_str(input: &str) -> Result<Usage, Self::Err> {
        match input {
            "Counter" => Ok(Usage::Counter),
            "Gauge" => Ok(Usage::Gauge),
            "Histogram" => Ok(Usage::Histogram),
            "Label" => Ok(Usage::Label),
            _ => Err(()),
        }
    }
}


pub async fn create_postgres_exporter_role(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    if !(cdb.spec.postgresExporterEnabled) {
        return Ok(());
    }
    debug!(
        "Creating postgres_exporter role for database {} in namespace {}",
        cdb.metadata.name.clone().unwrap(),
        cdb.metadata.namespace.clone().unwrap()
    );
    // https://github.com/prometheus-community/postgres_exporter#running-as-non-superuser
    let _ = cdb
        .psql(
            "
            CREATE OR REPLACE FUNCTION __tmp_create_user() returns void as $$
            BEGIN
              IF NOT EXISTS (
                      SELECT
                      FROM   pg_catalog.pg_user
                      WHERE  usename = 'postgres_exporter') THEN
                CREATE USER postgres_exporter;
              END IF;
            END;
            $$ language plpgsql;

            SELECT __tmp_create_user();
            DROP FUNCTION __tmp_create_user();

            ALTER USER postgres_exporter SET SEARCH_PATH TO postgres_exporter,pg_catalog;
            GRANT CONNECT ON DATABASE postgres TO postgres_exporter;
            GRANT pg_monitor to postgres_exporter;
            GRANT pg_read_all_stats to postgres_exporter;
            "
            .to_string(),
            "postgres".to_owned(),
            client.clone(),
        )
        .await?;
    Ok(())
}
