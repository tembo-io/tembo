use crate::{
    controller::{Extension, ExtensionInstallLocation},
    Context, CoreDB, Error,
};
use regex::Regex;
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct ExtRow {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub schema: String,
}

// contains all the extensions in a database
pub struct DbExt {
    pub dbname: String,
    pub extensions: Vec<ExtRow>,
}

const LIST_DATABASES_QUERY: &str = r#"SELECT datname FROM pg_database WHERE datistemplate = false;"#;
const LIST_EXTENSIONS_QUERY: &str = r#"select
distinct on
(name) *
from
(
select
    *
from
    (
    select
        t0.extname as name,
        t0.extversion as version,
        true as enabled,
        t1.nspname as schema
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
    ) t1
    where
        t1.oid = t0.extnamespace
) installed
union
select 
    name,
    default_version as version,
    false as enabled,
    'public' as schema
from
    pg_catalog.pg_available_extensions
order by
    enabled asc 
) combined
order by
name asc,
enabled desc
"#;

pub async fn manage_extensions(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let extensions = &cdb.spec.extensions;
    let re = Regex::new(r"[a-zA-Z][0-9a-zA-Z_-]*$").unwrap();

    // TODO(ianstanton) Some extensions will fail to create. We need to handle and surface any errors.
    //  Logging result at debug level for now.

    // iterate through list of extensions and run CREATE EXTENSION <extension-name> for each
    for ext in extensions {
        let ext_name = ext.name.as_str();
        if !re.is_match(ext_name) {
            warn!(
                "Extension {} is not formatted properly. Skipping operation.",
                ext_name
            )
        } else {
            for ext_loc in ext.locations.iter() {
                let database_name = ext_loc.database.to_owned();
                if !re.is_match(&database_name) {
                    warn!(
                        "Extension.Database {}.{} is not formatted properly. Skipping operation.",
                        ext_name, database_name
                    );
                    continue;
                }
                if ext_loc.enabled {
                    info!("Creating extension: {}, database {}", ext_name, database_name);
                    let schema_name = ext_loc.schema.to_owned();
                    if !re.is_match(&schema_name) {
                        warn!(
                            "Extension.Database.Schema {}.{}.{} is not formatted properly. Skipping operation.",
                            ext_name, database_name, schema_name
                        );
                        continue;
                    }
                    // this will no-op if we've already created the extension
                    let result = cdb
                        .psql(
                            format!("CREATE EXTENSION IF NOT EXISTS {ext_name} SCHEMA {schema_name};"),
                            database_name,
                            client.clone(),
                        )
                        .await
                        .unwrap();
                    debug!("Result: {}", result.stdout.clone().unwrap());
                } else {
                    info!("Dropping extension: {}, database {}", ext_name, database_name);
                    let result = cdb
                        .psql(
                            format!("DROP EXTENSION IF EXISTS {ext_name};"),
                            database_name,
                            client.clone(),
                        )
                        .await
                        .unwrap();
                    debug!("Result: {}", result.stdout.clone().unwrap());
                }
            }
        }
    }
    Ok(())
}

pub async fn exec_list_databases(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<String>, Error> {
    let client = ctx.client.clone();
    let psql_out = cdb
        .psql(
            LIST_DATABASES_QUERY.to_owned(),
            "postgres".to_owned(),
            client.clone(),
        )
        .await
        .unwrap();
    let result_string = psql_out.stdout.unwrap();
    let mut databases = vec![];
    for line in result_string.lines().skip(2) {
        let fields: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        if fields.len() < 4 {
            debug!("Done:{:?}", fields);
            continue;
        }
        databases.push(fields[0].to_string());
    }
    let num_databases = databases.len();
    info!("Found {} databases", num_databases);
    Ok(databases)
}

pub async fn exec_list_extensions(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    database: &str,
) -> Result<Vec<ExtRow>, Error> {
    let client = ctx.client.clone();
    let psql_out = cdb
        .psql(
            LIST_EXTENSIONS_QUERY.to_owned(),
            database.to_owned(),
            client.clone(),
        )
        .await
        .unwrap();
    let result_string = psql_out.stdout.unwrap();
    let mut extensions = vec![];
    for line in result_string.lines().skip(2) {
        let fields: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        if fields.len() < 4 {
            debug!("Done:{:?}", fields);
            continue;
        }
        let package = ExtRow {
            name: fields[0].to_owned(),
            version: fields[1].to_owned(),
            enabled: fields[2] == "t",
            schema: fields[3].to_owned(),
        };
        extensions.push(package);
    }
    let num_extensions = extensions.len();
    info!("Found {} extensions", num_extensions);
    Ok(extensions)
}

// wrangle the extensions in installed
// return as the crd / spec
pub async fn exec_get_all_extensions(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<Extension>, Error> {
    let databases = exec_list_databases(cdb, ctx.clone()).await?;

    let mut ext_hashmap: HashMap<String, Vec<ExtensionInstallLocation>> = HashMap::new();
    for db in databases {
        let extensions = exec_list_extensions(cdb, ctx.clone(), &db).await?;
        for ext in extensions {
            let extlocation = ExtensionInstallLocation {
                database: db.clone(),
                version: Some(ext.version),
                enabled: ext.enabled,
                schema: ext.schema,
            };
            ext_hashmap
                .entry(ext.name)
                .or_insert_with(Vec::new)
                .push(extlocation);
        }
    }

    let mut ext_spec: Vec<Extension> = Vec::new();
    for (ext_name, ext_locations) in &ext_hashmap {
        ext_spec.push(Extension {
            name: ext_name.clone(),
            locations: ext_locations.clone(),
        });
    }
    Ok(ext_spec)
}
