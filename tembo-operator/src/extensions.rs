use crate::{apis::coredb_types::CoreDB, controller::patch_cdb_status_merge, defaults, Context, Error};

use kube::{api::Api, runtime::controller::Action};
use lazy_static::lazy_static;
use regex::Regex;
use schemars::JsonSchema;
use semver::Comparator;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

lazy_static! {
    static ref VALID_INPUT: Regex = Regex::new(r"^[a-zA-Z]([a-zA-Z0-9]*[-_]?)*[a-zA-Z0-9]+$").unwrap();
}


// mapping of extension names to their trunk project names
lazy_static! {
    pub static ref TRUNK_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("vector", "pgvector");
        m.insert("embedding", "pg_embedding");
        m.insert("pgml", "postgresml");
        m.insert("columnar", "hydra_columnar");
        m.insert("currency", "pg_currency");
        m
    };
}

// map the extension name to the trunk project, if a mapping exists
// otherwise, returns the extension name
fn get_trunk_project_name(ext_name: &str) -> String {
    match TRUNK_MAP.get(ext_name) {
        Some(name) => name.to_string(),
        None => ext_name.to_string(),
    }
}


#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct Extension {
    pub name: String,
    #[serde(default = "defaults::default_description")]
    pub description: Option<String>,
    pub locations: Vec<ExtensionInstallLocation>,
}

impl Default for Extension {
    fn default() -> Self {
        Extension {
            name: "pg_stat_statements".to_owned(),
            description: Some(
                " track planning and execution statistics of all SQL statements executed".to_owned(),
            ),
            locations: vec![ExtensionInstallLocation::default()],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Serialize, PartialEq)]
pub struct ExtensionInstallLocation {
    pub enabled: bool,
    // no database or schema when disabled
    #[serde(default = "defaults::default_database")]
    pub database: String,
    // schema is optional. Some extensions get installed into their own schema and is handled by the extension
    pub schema: Option<String>,
    pub version: Option<String>,
}

impl Default for ExtensionInstallLocation {
    fn default() -> Self {
        ExtensionInstallLocation {
            schema: Some("public".to_owned()),
            database: "postgres".to_owned(),
            enabled: true,
            version: Some("1.9".to_owned()),
        }
    }
}

#[derive(Debug)]
pub struct ExtRow {
    pub name: String,
    pub description: String,
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

/// handles installing extensions
pub async fn install_extensions(
    cdb: &CoreDB,
    extensions: &[Extension],
    ctx: Arc<Context>,
) -> Result<(), Action> {
    debug!("extensions to install: {:?}", extensions);
    let client = ctx.client.clone();

    let cnpg_enabled = cdb.cnpg_enabled(ctx.clone()).await;

    let pod_name_cnpg = if cnpg_enabled {
        cdb.primary_pod_cnpg(client.clone())
            .await?
            .metadata
            .name
            .expect("Pod should always have a name")
    } else {
        return Err(Action::requeue(Duration::from_secs(300)));
    };

    let mut errors: Vec<Error> = Vec::new();
    let num_to_install = extensions.len();
    for ext in extensions.iter() {
        let version = ext.locations[0].version.clone().unwrap_or_else(|| {
            let err = Error::InvalidErr("Missing version for extension".to_string());
            error!("{}", err);
            err.to_string()
        });
        if !ext.locations[0].enabled {
            // If the extension is not enabled, don't bother trying to install it
            continue;
        }

        // determine appropriate trunk name
        let trunk_name = get_trunk_project_name(&ext.name);

        let cmd = vec![
            "trunk".to_owned(),
            "install".to_owned(),
            "-r https://registry.pgtrunk.io".to_owned(),
            trunk_name,
            "--version".to_owned(),
            version,
        ];

        let result = if !pod_name_cnpg.is_empty() {
            let cnpg_exec = cdb.exec(pod_name_cnpg.clone(), client.clone(), &cmd);
            cnpg_exec.await
        } else {
            continue;
        };

        match result {
            Ok(result) => {
                debug!("installed extension: {}", result.stdout.clone().unwrap());
            }
            Err(err) => {
                error!("error installing extension, {}", err);
                errors.push(err);
            }
        }
    }
    let num_success = num_to_install - errors.len();
    info!(
        "Successfully installed {} / {} extensions",
        num_success, num_to_install
    );
    Ok(())
}

/// handles create/drop extensions
pub async fn toggle_extensions(
    cdb: &CoreDB,
    extensions: &[Extension],
    ctx: Arc<Context>,
) -> Result<(), Action> {
    // iterate through list of extensions and run CREATE EXTENSION <extension-name> for each
    for ext in extensions {
        let ext_name = ext.name.as_str();
        if !check_input(ext_name) {
            warn!(
                "Extension {} is not formatted properly. Skipping operation.",
                ext_name
            )
        } else {
            // extensions can be installed in multiple databases but only a single schema
            for ext_loc in ext.locations.iter() {
                let database_name = ext_loc.database.clone();
                let command = match generate_extension_enable_cmd(ext_name, ext_loc) {
                    Ok(cmd) => cmd,
                    Err(e) => {
                        warn!(
                            "Failed to generate command for extension {}. Error: {:?}. Continuing.",
                            ext_name, e
                        );
                        continue;
                    }
                };

                let result = cdb
                    .psql(command.clone(), database_name.clone(), ctx.clone())
                    .await;

                match result {
                    Ok(_) => {}
                    Err(e) => {
                        // Even if one extension has failed to reconcile, we should
                        // still try to create the other extensions.
                        // It will retry on the next reconcile.
                        error!(
                            "Failed to reconcile extension {}, in {}. Error: {:?}. Ignoring.",
                            &ext_name,
                            cdb.metadata
                                .name
                                .clone()
                                .expect("instance should always have a name"),
                            e
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

/// generates the CREATE or DROP EXTENSION command for a given extension
/// handles schema specification in the command
fn generate_extension_enable_cmd(
    ext_name: &str,
    ext_loc: &ExtensionInstallLocation,
) -> Result<String, Error> {
    let database_name = ext_loc.database.to_owned();
    if !check_input(&database_name) {
        return Err(Error::InvalidErr(format!(
            "Extension.Database {}.{} is not formatted properly. Skipping operation.",
            ext_name, database_name
        )));
    }
    // only specify the schema if it provided
    let command = match ext_loc.enabled {
        true => {
            match ext_loc.schema.as_ref() {
                Some(schema) => {
                    if !check_input(&schema) {
                        return Err(Error::InvalidErr( format!("Extension.Database.Schema {}.{}.{} is not formatted properly. Skipping operation.",
                        ext_name, database_name, schema)));
                    }
                    format!(
                        "CREATE EXTENSION IF NOT EXISTS \"{}\" SCHEMA {} CASCADE;",
                        ext_name, schema
                    )
                }
                None => format!("CREATE EXTENSION IF NOT EXISTS \"{}\" CASCADE;", ext_name),
            }
        }
        false => format!("DROP EXTENSION IF EXISTS \"{}\" CASCADE;", ext_name),
    };
    Ok(command)
}

pub fn check_input(input: &str) -> bool {
    VALID_INPUT.is_match(input)
}

/// returns all the databases in an instance
pub async fn list_databases(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<String>, Action> {
    let _client = ctx.client.clone();
    let psql_out = cdb
        .psql(LIST_DATABASES_QUERY.to_owned(), "postgres".to_owned(), ctx)
        .await?;
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
pub async fn list_extensions(cdb: &CoreDB, ctx: Arc<Context>, database: &str) -> Result<Vec<ExtRow>, Action> {
    let psql_out = cdb
        .psql(LIST_EXTENSIONS_QUERY.to_owned(), database.to_owned(), ctx)
        .await?;
    let result_string = psql_out.stdout.unwrap();
    Ok(parse_extensions(&result_string))
}

fn parse_extensions(psql_str: &str) -> Vec<ExtRow> {
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

/// list databases then get all extensions from each database
pub async fn get_all_extensions(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Vec<Extension>, Action> {
    let databases = list_databases(cdb, ctx.clone()).await?;
    debug!("databases: {:?}", databases);

    let mut ext_hashmap: HashMap<(String, String), Vec<ExtensionInstallLocation>> = HashMap::new();
    // query every database for extensions
    // transform results by extension name, rather than by database
    for db in databases {
        let extensions = list_extensions(cdb, ctx.clone(), &db).await?;
        for ext in extensions {
            let extlocation = ExtensionInstallLocation {
                database: db.clone(),
                version: Some(ext.version),
                enabled: ext.enabled,
                schema: Some(ext.schema),
            };
            ext_hashmap
                .entry((ext.name, ext.description))
                .or_insert_with(Vec::new)
                .push(extlocation);
        }
    }

    let mut ext_spec: Vec<Extension> = Vec::new();
    for ((extname, extdescr), ext_locations) in &ext_hashmap {
        ext_spec.push(Extension {
            name: extname.clone(),
            description: Some(extdescr.clone()),
            locations: ext_locations.clone(),
        });
    }
    // put them in order
    ext_spec.sort_by_key(|e| e.name.clone());
    Ok(ext_spec)
}

// returns any elements that are in the desired, and not in actual
// any Extensions returned by this function need either create or drop extension
// cheap way to determine if there have been any sort of changes to extensions
fn diff_extensions(desired: &[Extension], actual: &[Extension]) -> Vec<Extension> {
    let set_desired: HashSet<_> = desired.iter().cloned().collect();
    let set_actual: HashSet<_> = actual.iter().cloned().collect();
    let mut diff: Vec<Extension> = set_desired.difference(&set_actual).cloned().collect();
    diff.sort_by_key(|e| e.name.clone());
    debug!("Extensions diff: {:?}", diff);
    diff
}

/// determines which extensions need create/drop and which need to be trunk installed
/// this is intended to be called after diff_extensions()
fn extension_plan(have_changed: &[Extension], actual: &[Extension]) -> (Vec<Extension>, Vec<Extension>) {
    let mut changed = Vec::new();
    let mut to_install = Vec::new();

    // have_changed is unlikely to ever be >10s of extensions
    for extension_desired in have_changed {
        // check if the extension name exists in the actual list
        let mut found = false;
        // actual unlikely to be > 100s of extensions
        for extension_actual in actual {
            if extension_desired.name == extension_actual.name {
                found = true;
                // extension exists, therefore has been installed
                // determine if the `enabled` toggle has changed
                'loc: for loc_desired in extension_desired.locations.clone() {
                    for loc_actual in extension_actual.locations.clone() {
                        if loc_desired.database == loc_actual.database {
                            if loc_desired.enabled != loc_actual.enabled {
                                debug!("desired: {:?}, actual: {:?}", extension_desired, extension_actual);
                                changed.push(extension_desired.clone());
                                break 'loc;
                            }

                            // Never need to install disabled extensions
                            if !loc_desired.enabled {
                                debug!("desired: {:?}, actual: {:?}", extension_desired, extension_actual);
                                break 'loc;
                            }

                            if loc_desired.version.is_some() {
                                let desired_version = Comparator::parse(
                                    &loc_desired
                                        .version
                                        .clone()
                                        .expect("Expected to find desired version"),
                                )
                                .expect("Failed to parse version into semver");
                                if loc_actual.version.is_some() {
                                    let actual_version = Comparator::parse(
                                        &loc_actual
                                            .version
                                            .clone()
                                            .expect("Expected to find desired version"),
                                    )
                                    .expect("Failed to parse version into semver");
                                    // If the major and minor versions do not match, then we need to install the extension
                                    if desired_version.major != actual_version.major
                                        || desired_version.minor != actual_version.minor
                                    {
                                        debug!(
                                            "desired: {:?}, actual: {:?}",
                                            extension_desired, extension_actual
                                        );
                                        to_install.push(extension_desired.clone());
                                        break 'loc;
                                    }
                                    // If the patch version exists on both and does not match, then we need to install the extension
                                    if desired_version.patch.is_some()
                                        && actual_version.patch.is_some()
                                        && desired_version.patch != actual_version.patch
                                    {
                                        debug!(
                                            "desired: {:?}, actual: {:?}",
                                            extension_desired, extension_actual
                                        );
                                        to_install.push(extension_desired.clone());
                                        break 'loc;
                                    }
                                } else {
                                    warn!("We desire a specific version of an extension {}, but the actual extension is not versioned. Skipping.", extension_desired.name);
                                }
                            } // Else, if the desired does not have a version and we already detected matching name, then do nothing
                        }
                    }
                }
            }
        }
        // if it doesn't exist, it needs to be installed
        if !found {
            to_install.push(extension_desired.clone());
        }
    }
    debug!(
        "extension to create/drop: {:?}, extensions to install: {:?}",
        changed, to_install
    );
    (changed, to_install)
}

/// reconcile extensions between the spec and the database
pub async fn reconcile_extensions(
    coredb: &CoreDB,
    ctx: Arc<Context>,
    cdb_api: &Api<CoreDB>,
    name: &str,
) -> Result<Vec<Extension>, Action> {
    // always get the current state of extensions in the database
    // this is due to out of band changes - manual create/drop extension
    let actual_extensions = get_all_extensions(coredb, ctx.clone()).await?;
    let mut desired_extensions = coredb.spec.extensions.clone();
    desired_extensions.sort_by_key(|e| e.name.clone());

    // most of the time there will be no changes
    let extensions_changed = diff_extensions(&desired_extensions, &actual_extensions);

    if extensions_changed.is_empty() {
        // no further work when no changes
        return Ok(actual_extensions);
    }

    // otherwise, need to determine the plan to apply
    let (changed_extensions, extensions_to_install) = extension_plan(&extensions_changed, &actual_extensions);

    if !changed_extensions.is_empty() || !extensions_to_install.is_empty() {
        let status = serde_json::json!({
            "status": {"extensionsUpdating": true}
        });
        // TODO: we should have better handling/behavior for when we fail to patch the status
        let _ = patch_cdb_status_merge(cdb_api, name, status).await;
        if !changed_extensions.is_empty() {
            toggle_extensions(coredb, &changed_extensions, ctx.clone()).await?;
        }
        if !extensions_to_install.is_empty() {
            install_extensions(coredb, &extensions_to_install, ctx.clone()).await?;
        }
        let status = serde_json::json!({
            "status": {"extensionsUpdating": false}
        });
        let _ = patch_cdb_status_merge(cdb_api, name, status).await;
    }
    // return final state of extensions
    get_all_extensions(coredb, ctx.clone()).await
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_get_trunk_project_name() {
        let pgvector = get_trunk_project_name("vector");
        assert_eq!(pgvector, "pgvector");
        let pgml = get_trunk_project_name("pgml");
        assert_eq!(pgml, "postgresml");
        let pgmq = get_trunk_project_name("pgmq");
        assert_eq!(pgmq, "pgmq");
        let dne = get_trunk_project_name("does_not_exist");
        assert_eq!(dne, "does_not_exist");
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

        // error mode: malformed database name
        let loc2 = ExtensionInstallLocation {
            database: "postgres; --".to_string(),
            enabled: true,
            schema: Some("public".to_string()),
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc2);
        assert!(cmd.is_err());

        // error mode: malformed schema name
        let loc2 = ExtensionInstallLocation {
            database: "postgres".to_string(),
            enabled: true,
            schema: Some("public; --".to_string()),
            version: Some("1.0.0".to_string()),
        };
        let cmd = generate_extension_enable_cmd("my_ext", &loc2);
        assert!(cmd.is_err());
    }

    #[test]
    fn test_extension_plan() {
        let aggs_for_vecs_disabled = Extension {
            name: "aggs_for_vecs".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.3.0".to_owned()),
            }],
        };

        let pgmq_disabled = Extension {
            name: "pgmq".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("0.9.0".to_owned()),
            }],
        };
        let diff = vec![pgmq_disabled.clone()];
        let actual = vec![aggs_for_vecs_disabled];
        let (changed, to_install) = extension_plan(&diff, &actual);
        assert!(changed.is_empty());
        assert!(to_install.len() == 1);

        let diff = vec![pgmq_disabled.clone()];
        let actual = vec![pgmq_disabled];
        let (changed, to_install) = extension_plan(&diff, &actual);
        assert!(changed.is_empty());
        assert!(to_install.is_empty());
    }

    #[test]
    fn test_extension_version_compare() {
        let pg_stat_statements_no_patch = Extension {
            name: "pg_stat_statements".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.10".to_owned()),
            }],
        };

        let pg_stat_statements_with_patch = Extension {
            name: "pg_stat_statements".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.10.0".to_owned()),
            }],
        };
        let diff = vec![pg_stat_statements_with_patch];
        let actual = vec![pg_stat_statements_no_patch];
        let (changed, to_install) = extension_plan(&diff, &actual);
        assert!(changed.is_empty());
        assert!(to_install.is_empty());
    }

    #[test]
    fn test_diff_and_plan() {
        let postgis_disabled = Extension {
            name: "postgis".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.1.1".to_owned()),
            }],
        };
        let postgis_enabled = Extension {
            name: "postgis".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.1.1".to_owned()),
            }],
        };
        let pgmq_disabled = Extension {
            name: "pgmq".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.1.1".to_owned()),
            }],
        };
        let pg_stat_enabled = Extension {
            name: "pg_stat_statements".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.1.1".to_owned()),
            }],
        };
        // three desired
        let desired = vec![
            postgis_disabled.clone(),
            pgmq_disabled.clone(),
            pg_stat_enabled.clone(),
        ];
        // two currently installed
        let actual = vec![postgis_enabled, pgmq_disabled];
        // postgis changed from enabled to disabled, and pg_stat is added
        // no change to pgmq

        // determine which extensions have changed or are new
        let diff = diff_extensions(&desired, &actual);
        assert!(
            diff.len() == 2,
            "expected two changed extensions, found extensions {:?}",
            diff
        );
        // should be postgis and pg_stat that are the diff
        assert_eq!(diff[0], pg_stat_enabled, "expected pg_stat, found {:?}", diff[0]);
        assert_eq!(diff[1], postgis_disabled, "expected postgis, found {:?}", diff[1]);
        // determine which of these are is a change and which is an install op
        let (changed, to_install) = extension_plan(&diff, &actual);
        assert_eq!(changed.len(), 1);
        assert!(
            changed[0] == postgis_disabled,
            "expected postgis changed to disabled, found {:?}",
            changed[0]
        );

        assert_eq!(to_install.len(), 1, "expected 1 install, found {:?}", to_install);
        assert!(
            to_install[0] == pg_stat_enabled,
            "expected pg_stat to install, found {:?}",
            to_install[0]
        );
    }

    #[test]
    fn test_upgrade_ext_vers() {
        let pgmq_05 = Extension {
            name: "pgmq".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("0.5.0".to_owned()),
            }],
        };

        let pgmq_06 = Extension {
            name: "pgmq".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("0.6.0".to_owned()),
            }],
        };
        let desired = vec![pgmq_06.clone()];
        let actual = vec![pgmq_05];
        // diff should be that we need to upgrade pgmq
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0], pgmq_06);

        // validate extension plan, should also be pgmq06
        let (changed, to_install) = extension_plan(&diff, &actual);
        assert_eq!(
            changed.len(),
            0,
            "expected no changed extensions, found {:?}",
            changed
        );
        assert_eq!(to_install.len(), 1, "expected 1 install, found {:?}", to_install);
    }

    #[test]
    fn test_diff() {
        let aggs_for_vecs_disabled = Extension {
            name: "aggs_for_vecs".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("1.5.2".to_owned()),
            }],
        };

        let pgmq_enabled = Extension {
            name: "pgmq".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: true,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("0.9.0".to_owned()),
            }],
        };

        let pgmq_disabled = Extension {
            name: "pgmq".to_owned(),
            description: Some("my description".to_owned()),
            locations: vec![ExtensionInstallLocation {
                enabled: false,
                database: "postgres".to_owned(),
                schema: Some("public".to_owned()),
                version: Some("0.9.0".to_owned()),
            }],
        };

        // case where there are extensions in db but not on spec
        // happens on startup, for example
        let desired = vec![];
        let actual = vec![aggs_for_vecs_disabled.clone(), pgmq_enabled.clone()];
        // diff should be that we need to enable pgmq
        let diff = diff_extensions(&desired, &actual);
        assert!(diff.is_empty());

        let desired = vec![aggs_for_vecs_disabled.clone(), pgmq_enabled.clone()];
        let actual = vec![aggs_for_vecs_disabled.clone(), pgmq_disabled.clone()];
        // diff should be that we need to enable pgmq
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0], pgmq_enabled);

        // order does not matter
        let desired = vec![pgmq_enabled.clone(), aggs_for_vecs_disabled.clone()];
        let actual = vec![aggs_for_vecs_disabled.clone(), pgmq_disabled.clone()];
        // diff will still be to enable pgmq
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0], pgmq_enabled);

        let desired = vec![aggs_for_vecs_disabled.clone(), pgmq_enabled.clone()];
        let actual = vec![aggs_for_vecs_disabled.clone(), pgmq_disabled];
        // diff should be that we need to enable pgmq
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0], pgmq_enabled);

        let desired = vec![aggs_for_vecs_disabled.clone(), pgmq_enabled.clone()];
        let actual = vec![aggs_for_vecs_disabled.clone(), pgmq_enabled.clone()];
        // diff == actual, so diff should be empty
        let diff = diff_extensions(&desired, &actual);
        assert_eq!(diff.len(), 0);

        let desired = vec![aggs_for_vecs_disabled.clone()];
        let actual = vec![aggs_for_vecs_disabled, pgmq_enabled];
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
}
