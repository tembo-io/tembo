use crate::{
    controller::{Extension, ExtensionInstallLocation},
    Context, CoreDB, Error,
};
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tracing::{debug, info, warn};


#[derive(Debug)]
pub struct ExtRow {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub schema: String,
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

/// handles create/drop extensions
pub async fn toggle_extensions(
    cdb: &CoreDB,
    extensions: &[Extension],
    ctx: Arc<Context>,
) -> Result<(), Error> {
    let client = ctx.client.clone();
    let re = Regex::new(r"[a-zA-Z][0-9a-zA-Z_-]*$").unwrap();

    // iterate through list of extensions and run CREATE EXTENSION <extension-name> for each
    for ext in extensions {
        let ext_name = ext.name.as_str();
        if !re.is_match(ext_name) {
            warn!(
                "Extension {} is not formatted properly. Skipping operation.",
                ext_name
            )
        } else {
            // extensions can be installed in multiple databases but only a single schema
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

/// returns all the databases in an instance
pub async fn list_databases(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<String>, Error> {
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
    Ok(parse_databases(&result_string))
}

fn parse_databases(psql_str: &str) -> Vec<String> {
    let mut databases = vec![];
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
        databases.push(fields[0].to_string());
    }
    let num_databases = databases.len();
    info!("Found {} databases", num_databases);
    databases
}

/// lists all extensions in a single database
pub async fn list_extensions(cdb: &CoreDB, ctx: Arc<Context>, database: &str) -> Result<Vec<ExtRow>, Error> {
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
    Ok(parse_extensions(&result_string))
}

fn parse_extensions(psql_str: &str) -> Vec<ExtRow> {
    let mut extensions = vec![];
    for line in psql_str.lines().skip(2) {
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
    extensions
}

/// list databases then get all extensions from each database
pub async fn get_all_extensions(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<Extension>, Error> {
    let databases = list_databases(cdb, ctx.clone()).await?;
    debug!("databases: {:?}", databases);
    // sleep 10
    let mut ext_hashmap: HashMap<String, Vec<ExtensionInstallLocation>> = HashMap::new();
    // query every database for extensions
    // transform results by extension name, rather than by database
    for db in databases {
        let extensions = list_extensions(cdb, ctx.clone(), &db).await?;
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


/// reconcile extensions between the spec and the database
pub async fn reconcile_extensions(coredb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<Extension>, Error> {
    // always get the current state of extensions in the database
    // this is due to out of band changes - manual create/drop extension
    let actual_extensions = get_all_extensions(coredb, ctx.clone()).await?;
    let desired_extensions = coredb.spec.extensions.clone();

    let diff = diff_extensions(&desired_extensions, &actual_extensions);
    toggle_extensions(coredb, &diff, ctx.clone()).await?;

    // return final state of extensions
    get_all_extensions(coredb, ctx.clone()).await
}


// returns any elements that are in the desired, and not in actual
// any Extensions returned by this function need to get "applied"
fn diff_extensions(desired: &[Extension], actual: &[Extension]) -> Vec<Extension> {
    let set_desired: HashSet<_> = desired.iter().cloned().collect();
    let set_actual: HashSet<_> = actual.iter().cloned().collect();
    let diff: Vec<Extension> = set_desired.difference(&set_actual).cloned().collect();
    info!("Extensions diff: {:?}", diff);
    diff
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff() {
        let postgis_disabled = Extension {
            name: "postgis".to_owned(),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: "public".to_owned(),
                version: Some("1.1.1".to_owned()),
            }],
        };

        let pgmq_enabled = Extension {
            name: "pgmq".to_owned(),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: "public".to_owned(),
                version: Some("1.1.1".to_owned()),
            }],
        };

        let pgmq_disabled = Extension {
            name: "pgmq".to_owned(),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: "public".to_owned(),
                version: Some("1.1.1".to_owned()),
            }],
        };

        let desired = vec![postgis_disabled.clone(), pgmq_enabled.clone()];
        let actual = vec![postgis_disabled.clone(), pgmq_disabled.clone()];
        // diff should be that we need to enable pgmq
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0], pgmq_enabled);

        // order does not matter
        let desired = vec![pgmq_enabled.clone(), postgis_disabled.clone()];
        let actual = vec![postgis_disabled.clone(), pgmq_disabled.clone()];
        // diff will still be to enable pgmq
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0], pgmq_enabled);

        let desired = vec![postgis_disabled.clone(), pgmq_enabled.clone()];
        let actual = vec![postgis_disabled.clone(), pgmq_disabled.clone()];
        // diff should be that we need to enable pgmq
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0], pgmq_enabled);

        let desired = vec![postgis_disabled.clone(), pgmq_enabled.clone()];
        let actual = vec![postgis_disabled.clone(), pgmq_enabled.clone()];
        // diff == actual, so diff should be empty
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 0);

        let desired = vec![postgis_disabled.clone()];
        let actual = vec![postgis_disabled.clone(), pgmq_enabled.clone()];
        // less extensions desired than exist - should be a no op
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 0);
    }


    #[test]
    fn test_parse_databases() {
        let three_db = " datname  
        ----------
         postgres
         cat
         dog
        (3 rows)
        
         ";

        let rows = parse_databases(three_db);
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

        let rows = parse_databases(one_db);
        println!("{:?}", rows);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], "postgres");
        assert_eq!(rows[1], "cat");
        assert_eq!(rows[2], "dog");
    }

    #[test]
    fn test_parse_extensions() {
        let ext_psql = "        name        | version | enabled |   schema   
        --------------------+---------+---------+------------
         adminpack          | 2.1     | f       | public
         amcheck            | 1.3     | f       | public
         autoinc            | 1.0     | f       | public
         bloom              | 1.0     | f       | public
         btree_gin          | 1.3     | f       | public
         btree_gist         | 1.7     | f       | public
         citext             | 1.6     | f       | public
         cube               | 1.5     | f       | public
         dblink             | 1.2     | f       | public";


        let ext = parse_extensions(ext_psql);
        assert_eq!(ext.len(), 9);
        assert_eq!(ext[0].name, "adminpack");
        assert_eq!(ext[0].enabled, false);
        assert_eq!(ext[0].version, "2.1".to_owned());
        assert_eq!(ext[0].schema, "public".to_owned());

        assert_eq!(ext[8].name, "dblink");
        assert_eq!(ext[8].enabled, false);
        assert_eq!(ext[8].version, "1.2".to_owned());
        assert_eq!(ext[8].schema, "public".to_owned());
    }
}
