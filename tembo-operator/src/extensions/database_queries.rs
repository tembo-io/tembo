use crate::{
    apis::{
        coredb_types::CoreDB,
        postgres_parameters::{ConfigValue, PgConfig},
    },
    extensions::{
        types,
        types::{ExtensionInstallLocation, ExtensionInstallLocationStatus, ExtensionStatus},
    },
    Context, RESTARTED_AT,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::DeleteParams, runtime::controller::Action, Api, ResourceExt};
use lazy_static::lazy_static;
use regex::Regex;
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};
use tracing::{debug, error, info, instrument, trace, warn};

lazy_static! {
    static ref VALID_INPUT: Regex =
        Regex::new(r"^[a-zA-Z]([a-zA-Z0-9]*[-_]?)*[a-zA-Z0-9]+$").unwrap();
}

pub fn check_input(input: &str) -> bool {
    VALID_INPUT.is_match(input)
}

pub const LIST_SHARED_PRELOAD_LIBRARIES_QUERY: &str = r#"SHOW shared_preload_libraries;"#;

pub const LIST_DATABASES_QUERY: &str =
    r#"SELECT datname FROM pg_database WHERE datname != 'template0';"#;

pub const LIST_EXTENSIONS_QUERY: &str = r#"select
distinct on
(name) *
from
(
select
    name,
    version,
    enabled,
    schema,
    description
from
    (
    select
        t0.extname as name,
        t0.extversion as version,
        true as enabled,
        t1.nspname as schema,
        comment as description
    from
        (
        select
            extnamespace,
            extname,
            extversion
        from
            pg_extension
) t0,
        (
        select
            oid,
            nspname
        from
            pg_namespace
) t1,
        (
        select
            name,
            comment
        from
            pg_catalog.pg_available_extensions
) t2
    where
        t1.oid = t0.extnamespace
        and t2.name = t0.extname
) installed
union
select
    name,
    default_version as version,
    false as enabled,
    'public' as schema,
    comment as description
from
    pg_catalog.pg_available_extensions
order by
    enabled asc
) combined
order by
name asc,
enabled desc
"#;

#[derive(Debug)]
pub struct ExtRow {
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub schema: String,
}

#[instrument(skip(cdb, ctx), fields(cdb_name = %cdb.name_any()))]
pub async fn list_shared_preload_libraries(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<String>, Action> {
    let psql_out = cdb
        .psql(
            LIST_SHARED_PRELOAD_LIBRARIES_QUERY.to_owned(),
            "postgres".to_owned(),
            ctx,
        )
        .await?;
    let result_string = match psql_out.stdout {
        None => {
            error!(
                "No stdout from psql when looking for shared_preload_libraries for {}",
                cdb.metadata.name.clone().unwrap()
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
        Some(out) => out,
    };
    let result = parse_sql_output(&result_string);
    let mut libraries: Vec<String> = vec![];
    if result.len() == 1 {
        libraries = result[0].split(',').map(|s| s.trim().to_string()).collect();
    }
    debug!(
        "{}: Found shared_preload_libraries: {:?}",
        cdb.metadata.name.clone().unwrap(),
        libraries.clone()
    );
    Ok(libraries)
}

/// lists all extensions in a single database
#[instrument(skip(cdb, ctx), fields(cdb_name = %cdb.name_any()))]
pub async fn list_extensions(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    database: &str,
) -> Result<Vec<ExtRow>, Action> {
    let psql_out = cdb
        .psql(LIST_EXTENSIONS_QUERY.to_owned(), database.to_owned(), ctx)
        .await?;
    let result_string = psql_out.stdout.unwrap();
    Ok(parse_extensions(&result_string))
}

/// List all configuration parameters
#[instrument(skip(cdb, ctx), fields(cdb_name = %cdb.name_any()))]
pub async fn list_config_params(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<PgConfig>, Action> {
    let psql_out = cdb
        .psql("SHOW ALL;".to_owned(), "postgres".to_owned(), ctx)
        .await?;
    let result_string = match psql_out.stdout {
        None => {
            error!(
                "No stdout from psql when looking for config values for {}",
                cdb.metadata.name.clone().unwrap()
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
        Some(out) => out,
    };
    Ok(parse_config_params(&result_string))
}

/// Returns Ok if the given database is running (i.e. not restarting)
#[instrument(skip(cdb, ctx), fields(cdb_name = %cdb.name_any()))]
pub async fn is_not_restarting(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    database: &str,
) -> Result<Option<DateTime<Utc>>, Action> {
    // chrono strftime declaration to parse Postgres timestamps
    const PG_TIMESTAMP_DECL: &str = "%Y-%m-%d %H:%M:%S.%f%#z";

    fn parse_psql_output(output: &str) -> Option<&str> {
        output.lines().nth(2).map(str::trim)
    }

    let cdb_name = cdb.name_any();

    let pg_postmaster_result = cdb
        .psql(
            "select pg_postmaster_start_time();".to_owned(),
            database.to_owned(),
            ctx.clone(),
        )
        .await;

    let Some(restarted_at) = cdb.annotations().get(RESTARTED_AT) else {
        // We don't have the annotation, so we are not restarting
        // return pg_postmaster_start_time if we have it.
        let result = pg_postmaster_result
            .ok()
            .and_then(|result| result.stdout)
            .as_ref() // Convert String to &str for parse_psql_output
            .and_then(|stdout| parse_psql_output(stdout))
            .and_then(|pg_postmaster_start_time_str| {
                let pg_postmaster_start_time = pg_postmaster_start_time_str.to_string();
                DateTime::parse_from_str(&pg_postmaster_start_time, PG_TIMESTAMP_DECL)
                    .ok()
                    .map(|dt_with_offset| dt_with_offset.with_timezone(&Utc))
            });
        return Ok(result);
    };

    let restarted_requested_at: DateTime<Utc> = DateTime::parse_from_rfc3339(restarted_at)
        .map_err(|err| {
            error!("{cdb_name}: Failed to deserialize DateTime from `restartedAt`: {err}");

            Action::requeue(Duration::from_secs(300))
        })?
        .into();

    let pg_postmaster = match pg_postmaster_result {
        Ok(result) => result.stdout.ok_or_else(|| {
            error!("{cdb_name}: select pg_postmaster_start_time() had no stdout");
            Action::requeue(Duration::from_secs(300))
        })?,
        Err(_) => {
            let pod = cdb
                .primary_pod_cnpg_ready_or_not(ctx.client.clone())
                .await?;

            let pod_not_ready_duration = match get_pod_not_ready_duration(pod.clone()) {
                Ok(Some(duration)) => {
                    warn!("Primary pod has not been ready for {:?}", duration);
                    duration
                }
                Ok(None) => {
                    warn!("{cdb_name}: Primary pod is ready or doesn't have a Ready condition, but we could not execute a command.");
                    return Err(Action::requeue(Duration::from_secs(5)));
                }
                Err(_e) => {
                    error!(
                        "{cdb_name}: Failed to determine how long the primary has not been ready"
                    );
                    return Err(Action::requeue(Duration::from_secs(300)));
                }
            };

            let pod_creation_timestamp = pod.metadata.creation_timestamp.ok_or_else(|| {
                error!("{cdb_name}: Pod has no creation timestamp");
                Action::requeue(Duration::from_secs(300))
            })?;

            let pod_age = Utc::now() - pod_creation_timestamp.0;

            // Check if the pod is older than restarted_at, and the pod has been not ready for over 30 seconds
            if pod_age > restarted_requested_at - Utc::now()
                && pod_not_ready_duration > Duration::from_secs(30)
            {
                error!("{cdb_name}: Primary pod is older than restarted_at and has been not ready for over 30 seconds. Deleting the pod");
                let pods_api =
                    Api::<Pod>::namespaced(ctx.client.clone(), &pod.metadata.namespace.unwrap());
                let delete_result = pods_api
                    .delete(&pod.metadata.name.unwrap(), &DeleteParams::default())
                    .await;
                if let Err(e) = delete_result {
                    error!("{cdb_name}: Failed to delete primary pod: {:?}", e);
                    return Err(Action::requeue(Duration::from_secs(300)));
                }
                return Err(Action::requeue(Duration::from_secs(10)));
            }
            return Err(Action::requeue(Duration::from_secs(15)));
        }
    };

    let pg_postmaster_start_time = parse_psql_output(&pg_postmaster).ok_or_else(|| {
        error!("{cdb_name}: failed to parse pg_postmaster_start_time() output");

        Action::requeue(Duration::from_secs(300))
    })?;

    let server_started_at: DateTime<Utc> = DateTime::parse_from_str(pg_postmaster_start_time, PG_TIMESTAMP_DECL)
        .map_err(|err| {
            tracing::error!(
                "{cdb_name}: Failed to deserialize DateTime from `pg_postmaster_start_time`: {err}, received '{pg_postmaster_start_time}'"
            );

            Action::requeue(Duration::from_secs(300))
        })?
        .into();

    if server_started_at >= restarted_requested_at {
        // Server started after the moment we requested it to restart,
        // meaning the restart is done
        debug!("Restart is complete for {}", cdb_name);
        Ok(Some(server_started_at))
    } else {
        // Server hasn't even started restarting yet
        warn!("Restart is not complete for {}, requeuing", cdb_name);
        Err(Action::requeue(Duration::from_secs(5)))
    }
}

fn get_pod_not_ready_duration(pod: Pod) -> Result<Option<Duration>, Box<dyn std::error::Error>> {
    let status = pod.status.ok_or("Pod has no status information")?;
    if let Some(conditions) = status.conditions {
        for condition in conditions {
            if condition.type_ == "Ready" {
                if condition.status == "False" {
                    // Extract the last transition time when the pod was not ready
                    let last_transition = condition
                        .last_transition_time
                        .ok_or("No last transition time for Ready condition")?;
                    let last_not_ready_time = last_transition.0;
                    let duration_since_not_ready = Utc::now() - last_not_ready_time;

                    let std_duration = match duration_since_not_ready.to_std() {
                        Ok(duration) => duration,
                        Err(_) => {
                            error!("Failed to convert duration to std::time::Duration");
                            return Ok(None);
                        }
                    };

                    return Ok(Some(std_duration));
                }
                break;
            }
        }
    }
    Ok(None)
}

#[instrument(skip(psql_str))]
pub fn parse_extensions(psql_str: &str) -> Vec<ExtRow> {
    let mut extensions = vec![];
    for line in psql_str.lines().skip(2) {
        let fields: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        if fields.len() < 5 {
            debug!("Done:{:?}", fields);
            continue;
        }
        let package = ExtRow {
            name: fields[0].to_owned(),
            version: fields[1].to_owned(),
            enabled: fields[2] == "t",
            schema: fields[3].to_owned(),
            description: fields[4].to_owned(),
        };
        extensions.push(package);
    }
    let num_extensions = extensions.len();
    debug!("Found {} extensions", num_extensions);
    extensions
}

/// returns all the databases in an instance
#[instrument(skip(cdb, ctx), fields(cdb_name = %cdb.name_any()))]
pub async fn list_databases(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<String>, Action> {
    let _client = ctx.client.clone();
    let psql_out = cdb
        .psql(LIST_DATABASES_QUERY.to_owned(), "postgres".to_owned(), ctx)
        .await?;
    let result_string = psql_out.stdout.unwrap();
    Ok(parse_sql_output(&result_string))
}

#[instrument(skip(psql_str))]
pub fn parse_sql_output(psql_str: &str) -> Vec<String> {
    let mut results = vec![];
    for line in psql_str.lines().skip(2) {
        let fields: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        if fields.is_empty()
            || fields[0].is_empty()
            || fields[0].contains("rows)")
            || fields[0].contains("row)")
        {
            debug!("Done:{:?}", fields);
            continue;
        }
        results.push(fields[0].to_string());
    }
    let num_results = results.len();
    debug!("Found {} results", num_results);
    results
}

/// Parse the output of `SHOW ALL` to get the parameter and its value. Return Vec<PgConfig>
#[instrument(skip(psql_str))]
pub fn parse_config_params(psql_str: &str) -> Vec<PgConfig> {
    let mut results = vec![];
    for line in psql_str.lines().skip(2) {
        let fields: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        if fields.len() < 2 {
            debug!("Skipping last line:{:?}", fields);
            continue;
        }
        // If value is multiple, Set as ConfigValue::Multiple
        if fields[1].contains(',') {
            let values: BTreeSet<String> =
                fields[1].split(',').map(|s| s.trim().to_owned()).collect();
            let config = PgConfig {
                name: fields[0].to_owned(),
                value: ConfigValue::Multiple(values),
            };
            results.push(config);
            continue;
        }
        let config = PgConfig {
            name: fields[0].to_owned(),
            value: ConfigValue::Single(fields[1].to_owned()),
        };
        results.push(config);
    }
    let num_results = results.len();
    debug!("Found {} config values", num_results);
    // Log config values to debug
    for result in &results {
        trace!("Config value: {:?}", result);
    }
    results
}

/// list databases then get all extensions from each database
#[instrument(skip(cdb, ctx), fields(cdb_name = %cdb.name_any()))]
pub async fn get_all_extensions(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<ExtensionStatus>, Action> {
    let databases = list_databases(cdb, ctx.clone()).await?;
    debug!("databases: {:?}", databases);

    let mut ext_hashmap: HashMap<(String, String), Vec<ExtensionInstallLocationStatus>> =
        HashMap::new();
    // query every database for extensions
    // transform results by extension name, rather than by database
    for db in databases {
        let extensions = list_extensions(cdb, ctx.clone(), &db).await?;
        for ext in extensions {
            let extlocation = ExtensionInstallLocationStatus {
                database: db.clone(),
                version: Some(ext.version),
                enabled: Some(ext.enabled),
                schema: Some(ext.schema),
                error: None,
                error_message: None,
            };
            ext_hashmap
                .entry((ext.name, ext.description))
                // .or_insert_with(Vec::new)
                .or_default()
                .push(extlocation);
        }
    }

    let mut ext_spec: Vec<ExtensionStatus> = Vec::new();
    for ((extname, extdescr), ext_locations) in &ext_hashmap {
        ext_spec.push(ExtensionStatus {
            name: extname.clone(),
            description: Some(extdescr.clone()),
            locations: ext_locations.clone(),
        });
    }
    // put them in order
    ext_spec.sort_by_key(|e| e.name.clone());

    Ok(ext_spec)
}

pub enum ToggleError {
    WithDescription(String),
    WithAction(Action),
}

/// Handles create/drop an extension location
/// On failure, returns an error message
#[instrument(skip(cdb, ctx), fields(cdb_name = %cdb.name_any(), ext_name, ext_loc))]
pub async fn toggle_extension(
    cdb: &CoreDB,
    ext_name: &str,
    ext_loc: ExtensionInstallLocation,
    ctx: Arc<Context>,
) -> Result<(), ToggleError> {
    let coredb_name = cdb.name_any();
    if !check_input(ext_name) {
        warn!(
            "Extension is not formatted properly. Skipping operation. {}",
            &coredb_name
        );
        return Err(ToggleError::WithDescription(
            "Extension name is not formatted properly".into(),
        ));
    }
    let database_name = ext_loc.database.to_owned();
    if !check_input(&database_name) {
        warn!(
            "Database name is not formatted properly. Skipping operation. {}",
            &coredb_name
        );
        return Err(ToggleError::WithDescription(
            "Database name is not formatted properly".into(),
        ));
    }

    let command = types::generate_extension_enable_cmd(ext_name, &ext_loc)
        .map_err(ToggleError::WithDescription)?;

    let result = cdb
        .psql(command.clone(), database_name.clone(), ctx.clone())
        .await;

    match result {
        Ok(psql_output) => match psql_output.success {
            true => {
                info!(
                    "Successfully toggled extension {} in database {}, instance {}",
                    ext_name, database_name, &coredb_name
                );
            }
            false => {
                warn!(
                    "Failed to toggle extension {} in database {}, instance {}",
                    ext_name, database_name, &coredb_name
                );
                match psql_output.stderr {
                    Some(stderr) => {
                        return Err(ToggleError::WithDescription(stderr));
                    }
                    None => {
                        return Err(ToggleError::WithDescription("Failed to enable extension, and found no output. Please try again. If this issue persists, contact support.".to_string()));
                    }
                }
            }
        },
        Err(e) => {
            error!(
                "Failed to reconcile extension because of kube exec error: {:?}",
                e
            );
            return Err(ToggleError::WithAction(e));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        apis::postgres_parameters::PgConfig,
        extensions::database_queries::{
            check_input, parse_config_params, parse_extensions, parse_sql_output,
        },
    };

    #[test]
    fn test_parse_databases() {
        let three_db = " datname
        ----------
         postgres
         cat
         dog
        (3 rows)

         ";

        let rows = parse_sql_output(three_db);
        println!("{:?}", rows);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], "postgres");
        assert_eq!(rows[1], "cat");
        assert_eq!(rows[2], "dog");

        let one_db = " datname
        ----------
         postgres
        (1 row)

         ";

        let rows = parse_sql_output(one_db);
        println!("{:?}", rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], "postgres");
    }

    #[test]
    fn test_parse_extensions() {
        let ext_psql = "        name        | version | enabled |   schema   |                              description
        --------------------+---------+---------+------------+------------------------------------------------------------------------
         adminpack          | 2.1     | f       | public     | administrative functions for PostgreSQL
         amcheck            | 1.3     | f       | public     | functions for verifying relation integrity
         autoinc            | 1.0     | f       | public     | functions for autoincrementing fields
         bloom              | 1.0     | f       | public     | bloom access method - signature file based index
         btree_gin          | 1.3     | f       | public     | support for indexing common datatypes in GIN
         btree_gist         | 1.7     | f       | public     | support for indexing common datatypes in GiST
         citext             | 1.6     | f       | public     | data type for case-insensitive character strings
         cube               | 1.5     | f       | public     | data type for multidimensional cubes
         dblink             | 1.2     | f       | public     | connect to other PostgreSQL databases from within a database
         (9 rows)";

        let ext = parse_extensions(ext_psql);
        assert_eq!(ext.len(), 9);
        assert_eq!(ext[0].name, "adminpack");
        assert!(!ext[0].enabled);
        assert_eq!(ext[0].version, "2.1".to_owned());
        assert_eq!(ext[0].schema, "public".to_owned());
        assert_eq!(
            ext[0].description,
            "administrative functions for PostgreSQL".to_owned()
        );

        assert_eq!(ext[8].name, "dblink");
        assert!(!ext[8].enabled);
        assert_eq!(ext[8].version, "1.2".to_owned());
        assert_eq!(ext[8].schema, "public".to_owned());
        assert_eq!(
            ext[8].description,
            "connect to other PostgreSQL databases from within a database".to_owned()
        );
    }

    #[test]
    fn test_parse_config_params() {
        let config_psql = "        name        | setting | unit | category | short_desc | extra_desc | context | vartype | source | min_val | max_val | enumvals | boot_val | reset_val | sourcefile | sourceline | pending_restart
        ---------------------+---------+------+----------+------------+------------+---------+---------+--------+---------+---------+----------+----------+-----------+------------+------------+-----------------
         allow_system_table_mods | off     |      | Developer |            |            | postmas | bool    |        |         |         |          | off      | off       |            |            | f
         application_name      |         |      |          |            |            | user    | string  |        |         |         |          |          |           |            |            |
         archive_command       |         |      |          |            |            | sighup  | string  |        |         |         |          |          |           |            |            |
         archive_mode          | off     |      |          |            |            | sighup  | enum    |        |         |         | on,off   | off      | off       |            |            | f";
        let config = parse_config_params(config_psql);
        assert_eq!(config.len(), 4);
        assert_eq!(
            config[0],
            PgConfig {
                name: "allow_system_table_mods".to_owned(),
                value: "off".parse().unwrap(),
            }
        );
        assert_eq!(
            config[1],
            PgConfig {
                name: "application_name".to_owned(),
                value: "".parse().unwrap(),
            }
        );
        assert_eq!(
            config[2],
            PgConfig {
                name: "archive_command".to_owned(),
                value: "".parse().unwrap(),
            }
        );
        assert_eq!(
            config[3],
            PgConfig {
                name: "archive_mode".to_owned(),
                value: "off".parse().unwrap(),
            }
        );
    }

    #[test]
    fn test_check_input() {
        let invalids = [
            "extension--",
            "data;",
            "invalid^#$$characters",
            ";invalid",
            "",
        ];
        for i in invalids.iter() {
            assert!(!check_input(i), "input {} should be invalid", i);
        }

        let valids = [
            "extension_a",
            "schema_abc",
            "extension",
            "NewExtension",
            "NewExtension123",
            "postgis_tiger_geocoder-3",
            "address_standardizer-3",
            "xml2",
        ];
        for i in valids.iter() {
            assert!(check_input(i), "input {} should be valid", i);
        }
    }
}
