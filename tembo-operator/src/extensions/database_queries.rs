use crate::{
    apis::coredb_types::CoreDB,
    extensions::types::{
        get_extension_status, ExtensionInstallLocation, ExtensionInstallLocationStatus, ExtensionStatus,
    },
    Context, Error,
};
use kube::runtime::controller::Action;
use lazy_static::lazy_static;
use regex::Regex;
use std::{collections::HashMap, sync::Arc, time::Duration};

use tracing::{debug, error, info, warn};

lazy_static! {
    static ref VALID_INPUT: Regex = Regex::new(r"^[a-zA-Z]([a-zA-Z0-9]*[-_]?)*[a-zA-Z0-9]+$").unwrap();
}

// TODO: Get this list from trunk instead of coding it here
pub const REQUIRES_LOAD: [&str; 22] = [
    "auth_delay",
    "auto_explain",
    "basebackup_to_shell",
    "basic_archive",
    "citus",
    "passwordcheck",
    "pg_anonymize",
    "pgaudit",
    "pg_cron",
    "pg_failover_slots",
    "pg_later",
    "pglogical",
    "pg_net",
    "pg_stat_kcache",
    "pg_stat_statements",
    "pg_tle",
    "plrust",
    "postgresql_anonymizer",
    "sepgsql",
    "supautils",
    "timescaledb",
    "vectorize",
];

pub fn check_input(input: &str) -> bool {
    VALID_INPUT.is_match(input)
}

pub const LIST_DATABASES_QUERY: &str = r#"SELECT datname FROM pg_database WHERE datistemplate = false;"#;

pub const LIST_SHARED_PRELOAD_LIBRARIES_QUERY: &str = r#"SHOW shared_preload_libraries;"#;

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

/// lists all extensions in a single database
pub async fn list_extensions_with_pg_available_extensions(
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

async fn list_installed_libraries(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<String>, Action> {
    let cmd = vec![
        "/bin/bash".to_string(),
        "-c".to_string(),
        "ls $(pg_config --libdir) | grep -E '.*\\.so.?.*' | cut -d'.' -f1 | uniq".to_string(),
    ];

    let client = ctx.client.clone();
    let pod_name = cdb
        .primary_pod_cnpg(client.clone())
        .await?
        .metadata
        .name
        .expect("Pod should always have a name");

    match cdb.exec(pod_name.clone(), client.clone(), &cmd).await {
        Ok(result) => match result.stdout {
            None => {
                error!(
                    "Failed to list installed libraries for {}, no stdout",
                    cdb.metadata
                        .name
                        .clone()
                        .expect("Database should always have a name")
                );
                Err(Action::requeue(Duration::from_secs(300)))
            }
            Some(stdout) => {
                let mut libraries = vec![];
                for line in stdout.lines() {
                    if !check_input(line) {
                        warn!("Found invalid library name: {}", line);
                        continue;
                    }
                    libraries.push(line.to_owned());
                }
                debug!(
                    "{} - found libraries: {:?}",
                    cdb.metadata.name.clone().unwrap(),
                    libraries
                );
                Ok(libraries)
            }
        },
        Err(_) => {
            warn!(
                "Failed to list installed libraries for {}, failed to exec",
                cdb.metadata
                    .name
                    .clone()
                    .expect("Database should always have a name")
            );
            Err(Action::requeue(Duration::from_secs(10)))
        }
    }
}

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

pub async fn list_databases(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<String>, Action> {
    let psql_out = cdb
        .psql(LIST_DATABASES_QUERY.to_owned(), "postgres".to_owned(), ctx)
        .await?;
    let result_string = psql_out.stdout.unwrap();
    Ok(parse_sql_output(&result_string))
}

pub async fn list_shared_preload_libraries(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<String>, Action> {
    let psql_out = cdb
        .psql(
            LIST_SHARED_PRELOAD_LIBRARIES_QUERY.to_owned(),
            "postgres".to_owned(),
            ctx,
        )
        .await?;
    let result_string = psql_out.stdout.unwrap();
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
    info!("Found {} results", num_results);
    results
}

/// list databases then get all extensions from each database
pub async fn get_all_extensions(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<ExtensionStatus>, Action> {
    let databases = list_databases(cdb, ctx.clone()).await?;
    debug!("databases: {:?}", databases);

    let mut ext_hashmap: HashMap<(String, String), Vec<ExtensionInstallLocationStatus>> = HashMap::new();
    // query every database for extensions
    // transform results by extension name, rather than by database
    for db in databases {
        let extensions = list_extensions_with_pg_available_extensions(cdb, ctx.clone(), &db).await?;
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
                .or_insert_with(Vec::new)
                .push(extlocation);
        }
    }

    let mut ext_spec: Vec<ExtensionStatus> = Vec::new();
    for ((extname, extdescr), ext_locations) in &ext_hashmap {
        // load is required if the extension name is present in REQUIRES_LOAD constant
        let load = REQUIRES_LOAD.contains(&extname.as_str());
        ext_spec.push(ExtensionStatus {
            name: extname.clone(),
            description: Some(extdescr.clone()),
            locations: ext_locations.clone(),
            // Extensions discovered by pg_available_extensions require create extension
            create_extension: Some(true),
            // Determine if load is required using metadata from Trunk
            load: Some(load),
        });
    }

    // Discover installed extensions that do not require create extension.
    // This is discovered by checking for any .so files in libdir that
    // match a name present in REQUIRES_LOAD, that are not already
    // covered by the pg_available_extensions query.
    let installed_libraries = list_installed_libraries(cdb, ctx.clone()).await?;
    let current_shared_preload_libraries = list_shared_preload_libraries(cdb, ctx.clone()).await?;
    for library in installed_libraries {
        if REQUIRES_LOAD.contains(&library.as_str()) {
            // If the library is already in the list, skip it
            if ext_spec.iter().any(|e| e.name == library) {
                continue;
            }
            // version is included if there is an exact name match between the extension
            // and a package in status.trunk_installs
            let version = get_version_of_installed_library(cdb, library.clone());
            // These extensions are considered enabled if present in shared_preload_libraries.
            let enabled = current_shared_preload_libraries.contains(&library);
            ext_spec.push(ExtensionStatus {
                name: library.clone(),
                description: None,
                locations: vec![ExtensionInstallLocationStatus {
                    // Even though technically not associated to a particular database,
                    // we add the default "postgres" so it will show up in the UI.
                    database: "postgres".to_string(),
                    version,
                    enabled: Some(enabled),
                    // Since schema is not applicable, we set it to "-"
                    schema: Some("-".to_string()),
                    error: None,
                    error_message: None,
                }],
                // Since we already omitted anything detected by pg_available_extensions,
                // that means this does not require CREATE EXTENSION
                create_extension: Some(false),
                // Since we checked if this libraries is in the REQUIRES_LOAD list,
                // we know that it requires load
                load: Some(true),
            });
        }
    }

    // put them in order
    ext_spec.sort_by_key(|e| e.name.clone());

    Ok(ext_spec)
}

fn get_version_of_installed_library(cdb: &CoreDB, library_name: String) -> Option<String> {
    // If the library name is an exact match to a package name, then we can
    // get the version from status.trunk_installs.
    // Improve this using trunk package -> extension name mapping,
    // The consequence is some extensions will not show a version.
    match &cdb.status {
        None => None,
        Some(status) => match &status.trunk_installs {
            None => None,
            Some(trunk_installs) => {
                for package in trunk_installs {
                    if package.name == library_name {
                        return package.version.clone();
                    }
                }
                None
            }
        },
    }
}

/// generates the CREATE or DROP EXTENSION command for a given extension
/// handles schema specification in the command
fn generate_extension_enable_cmd(
    ext_name: &str,
    ext_loc: &ExtensionInstallLocation,
) -> Result<String, Error> {
    // only specify the schema if it provided
    let command = match ext_loc.enabled {
        true => match ext_loc.schema.as_ref() {
            Some(schema) => {
                format!(
                    "CREATE EXTENSION IF NOT EXISTS \"{}\" SCHEMA {} CASCADE;",
                    ext_name, schema
                )
            }
            None => format!("CREATE EXTENSION IF NOT EXISTS \"{}\" CASCADE;", ext_name),
        },
        false => format!("DROP EXTENSION IF EXISTS \"{}\" CASCADE;", ext_name),
    };
    Ok(command)
}

/// Handles create/drop an extension location
/// On failure, returns an error message
pub async fn create_or_drop_extension_if_required(
    cdb: &CoreDB,
    ext_name: &str,
    ext_loc: ExtensionInstallLocation,
    ctx: Arc<Context>,
) -> Result<(), String> {
    let current_status = match get_extension_status(cdb, ext_name) {
        None => {
            error!("There should always be an extension status before attempting to toggle an extension");
            return Err("Extension is not installed".to_string());
        }
        Some(status) => status,
    };
    if current_status.create_extension.is_some() && !current_status.create_extension.unwrap() {
        // If the extension does not require CREATE EXTENSION, then we do not need to do anything in this function.
        return Ok(());
    }

    let coredb_name = cdb.metadata.name.clone().expect("CoreDB should have a name");
    if !check_input(ext_name) {
        warn!(
            "Extension is not formatted properly. Skipping operation. {}",
            &coredb_name
        );
        return Err("Extension name is not formatted properly".to_string());
    }
    let database_name = ext_loc.database.to_owned();
    if !check_input(&database_name) {
        warn!(
            "Database name is not formatted properly. Skipping operation. {}",
            &coredb_name
        );
        return Err("Database name is not formatted properly".to_string());
    }
    let schema_name = ext_loc.schema.to_owned();
    if schema_name.is_some() && !check_input(&schema_name.unwrap()) {
        warn!(
            "Extension.Database.Schema is not formatted properly. Skipping operation. {}",
            &coredb_name
        );
        return Err("Schema name is not formatted properly".to_string());
    }

    let command = match generate_extension_enable_cmd(ext_name, &ext_loc) {
        Ok(command) => command,
        Err(_) => {
            return Err(
                "Don't know how to enable this extension. You may enable the extension manually instead."
                    .to_string(),
            );
        }
    };

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
                match psql_output.stdout {
                    Some(stdout) => {
                        return Err(stdout);
                    }
                    None => {
                        return Err("Failed to enable extension, and found no output. Please try again. If this issue persists, contact support.".to_string());
                    }
                }
            }
        },
        Err(e) => {
            error!(
                "Failed to reconcile extension because of kube exec error: {:?}",
                e
            );
            return Err(
                "Could not connect to database, try again. If problem persists, please contact support."
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::extensions::{
        database_queries::{check_input, generate_extension_enable_cmd, parse_extensions, parse_sql_output},
        types::ExtensionInstallLocation,
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
    fn test_check_input() {
        let invalids = ["extension--", "data;", "invalid^#$$characters", ";invalid", ""];
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

    #[test]
    fn test_generate_extension_enable_cmd() {
        // schema not specified
        let loc1 = ExtensionInstallLocation {
            database: "postgres".to_string(),
            enabled: true,
            schema: None,
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc1);
        assert_eq!(cmd.unwrap(), "CREATE EXTENSION IF NOT EXISTS \"my_ext\" CASCADE;");

        // schema specified
        let loc2 = ExtensionInstallLocation {
            database: "postgres".to_string(),
            enabled: true,
            schema: Some("public".to_string()),
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc2);
        assert_eq!(
            cmd.unwrap(),
            "CREATE EXTENSION IF NOT EXISTS \"my_ext\" SCHEMA public CASCADE;"
        );

        // drop extension
        let loc2 = ExtensionInstallLocation {
            database: "postgres".to_string(),
            enabled: false,
            schema: Some("public".to_string()),
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc2);
        assert_eq!(cmd.unwrap(), "DROP EXTENSION IF EXISTS \"my_ext\" CASCADE;");
    }
}
