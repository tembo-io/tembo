use crate::{Context, CoreDB, Error};
use regex::Regex;
use std::sync::Arc;
use tracing::{debug, info, warn};

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
                if ext_loc.enabled {
                    info!("Creating extension: {}", ext_name);
                    // this will no-op if we've already created the extension
                    let result = cdb
                        .psql(
                            format!("CREATE EXTENSION IF NOT EXISTS {ext_name};"),
                            "postgres".to_owned(),
                            client.clone(),
                        )
                        .await
                        .unwrap();
                    debug!("Result: {}", result.stdout.clone().unwrap());
                } else {
                    info!("Dropping extension: {}", ext_name);
                    let result = cdb
                        .psql(
                            format!("DROP EXTENSION IF EXISTS {ext_name};"),
                            "postgres".to_owned(),
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
