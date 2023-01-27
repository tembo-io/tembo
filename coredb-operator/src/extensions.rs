use crate::{Context, CoreDB, Error};
use std::sync::Arc;
use tracing::debug;
use regex::Regex;

pub async fn create_extensions(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<(), Error> {
    let client = &ctx.client;
    let extensions = &cdb.spec.enabledExtensions;
    let re = Regex::new(r"[a-zA-Z][0-9a-zA-Z_-]*$").unwrap();

    // TODO(ianstanton) Some extensions will fail to create. We need to handle and surface any errors.
    //  Logging result at debug level for now.

    // iterate through list of extensions and run CREATE EXTENSION <extension-name> for each
    for ext in extensions {
        if !re.is_match(ext) {
            debug!("Extension {} is not formatted properly. Skipping creation.", ext)
        } else {
            debug!("Creating extension: {}", ext);
            // this will no-op if we've already created the extension
            let result = cdb
                .psql(
                    format!("CREATE EXTENSION {};", ext),
                    "postgres".to_owned(),
                    client.clone(),
                )
                .await
                .unwrap();
            debug!("Result: {}", result.stdout.clone().unwrap());
        }
    }
    Ok(())
}
