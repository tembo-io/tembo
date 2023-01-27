use crate::{Context, CoreDB, Error};
use std::sync::{atomic::compiler_fence, Arc};
use tracing::info;

pub async fn create_extensions(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<(), Error> {
    let client = &ctx.client;
    let extensions = &cdb.spec.enabledExtensions;

    // iterate through list of extensions and run CREATE EXTENSION <extension-name>
    // TODO(ianstanton) we need to check if the extensions are enabled before creating them
    for ext in extensions {
        info!("Creating extension: {}", ext);
        let result = cdb
            .psql(
                format!("CREATE EXTENSION {};", ext),
                "postgres".to_owned(),
                client.clone(),
            )
            .await
            .unwrap();
        println!("Result: {}", result.stdout.clone().unwrap());
    }
    Ok(())
}
