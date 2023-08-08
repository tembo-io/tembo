use crate::{
    apis::coredb_types::CoreDB,
    extensions::{
        kubernetes_queries::{add_trunk_install_to_status, remove_trunk_installs_from_status},
        types::{InstallStatus, TrunkInstall, TrunkInstallStatus},
    },
    Context,
};
use kube::{runtime::controller::Action, Api};
use std::{sync::Arc, time::Duration};
use tracing::{debug, error, info};

pub async fn reconcile_trunk_installs(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Vec<TrunkInstallStatus>, Action> {
    let coredb_api: Api<CoreDB> = Api::namespaced(
        ctx.client.clone(),
        &cdb.metadata
            .namespace
            .clone()
            .expect("CoreDB should have a namespace"),
    );

    // Get extensions in status.trunk_install that are not in spec
    // Deleting them from status allows for retrying installation
    // by first removing the extension from the spec, then adding it back
    let trunk_installs_to_remove_from_status = match &cdb.status {
        None => {
            vec![]
        }
        Some(status) => match &status.trunk_installs {
            None => {
                vec![]
            }
            Some(trunk_installs) => trunk_installs
                .iter()
                .filter(|&ext_status| {
                    !cdb.spec
                        .trunk_installs
                        .iter()
                        .any(|ext| ext.name == ext_status.name)
                })
                .collect::<Vec<_>>(),
        },
    };

    // Get list of names
    let trunk_install_names_to_remove_from_status = trunk_installs_to_remove_from_status
        .iter()
        .map(|ext_status| ext_status.name.clone())
        .collect::<Vec<_>>();

    // Remove extensions from status
    remove_trunk_installs_from_status(
        &coredb_api,
        &cdb.metadata.name.clone().expect("CoreDB should have a name"),
        trunk_install_names_to_remove_from_status,
    )
    .await?;

    // Get extensions in spec.trunk_install that are not in status.trunk_install
    let trunk_installs = cdb
        .spec
        .trunk_installs
        .iter()
        .filter(|&ext| {
            !cdb.status
                .clone()
                .unwrap_or_default()
                .trunk_installs
                .unwrap_or_default()
                .iter()
                .any(|ext_status| ext.name == ext_status.name)
        })
        .collect::<Vec<_>>();
    install_extensions(cdb, trunk_installs, ctx.clone()).await
}

/// handles installing extensions
pub async fn install_extensions(
    cdb: &CoreDB,
    trunk_installs: Vec<&TrunkInstall>,
    ctx: Arc<Context>,
) -> Result<Vec<TrunkInstallStatus>, Action> {
    let mut current_trunk_install_statuses: Vec<TrunkInstallStatus> = vec![];
    let coredb_name = cdb.metadata.name.clone().expect("CoreDB should have a name");
    info!("Installing extensions into {}: {:?}", coredb_name, trunk_installs);
    let client = ctx.client.clone();
    let coredb_api: Api<CoreDB> = Api::namespaced(
        ctx.client.clone(),
        &cdb.metadata
            .namespace
            .clone()
            .expect("CoreDB should have a namespace"),
    );

    let pod_name = cdb
        .primary_pod_cnpg(client.clone())
        .await?
        .metadata
        .name
        .expect("Pod should always have a name");

    let mut requeue = false;
    for ext in trunk_installs.iter() {
        let version = match ext.version.clone() {
            None => {
                error!(
                    "Installing extension {} into {}: missing version",
                    ext.name, coredb_name
                );
                let trunk_install_status = TrunkInstallStatus {
                    name: ext.name.clone(),
                    version: None,
                    status: InstallStatus::Error,
                    error_message: Some("Missing version".to_string()),
                };
                current_trunk_install_statuses =
                    add_trunk_install_to_status(&coredb_api, &coredb_name, &trunk_install_status).await?;
                continue;
            }
            Some(version) => version,
        };

        let cmd = vec![
            "trunk".to_owned(),
            "install".to_owned(),
            "-r https://registry.pgtrunk.io".to_owned(),
            ext.name.clone(),
            "--version".to_owned(),
            version,
        ];

        let result = cdb.exec(pod_name.clone(), client.clone(), &cmd).await;

        match result {
            Ok(result) => {
                let output = format!(
                    "stdout:\n{}\nstderr:\n{}",
                    result.stdout.clone().unwrap_or_default(),
                    result.stderr.clone().unwrap_or_default()
                );
                match result.success {
                    true => {
                        info!("Installed extension {} into {}", &ext.name, coredb_name);
                        debug!("{}", output);
                        let trunk_install_status = TrunkInstallStatus {
                            name: ext.name.clone(),
                            version: ext.version.clone(),
                            status: InstallStatus::Installed,
                            error_message: None,
                        };
                        current_trunk_install_statuses =
                            add_trunk_install_to_status(&coredb_api, &coredb_name, &trunk_install_status)
                                .await?
                    }
                    false => {
                        error!(
                            "Failed to install extension {} into {}:\n{}",
                            &ext.name,
                            coredb_name,
                            output.clone()
                        );
                        let trunk_install_status = TrunkInstallStatus {
                            name: ext.name.clone(),
                            version: ext.version.clone(),
                            status: InstallStatus::Error,
                            error_message: Some(output),
                        };
                        current_trunk_install_statuses =
                            add_trunk_install_to_status(&coredb_api, &coredb_name, &trunk_install_status)
                                .await?
                    }
                }
            }
            Err(err) => {
                // This kind of error means kube exec failed, which are errors other than the
                // trunk install command failing inside the pod. So, we should retry
                // when we find this kind of error.
                error!(
                    "Kube exec error installing extension {} into {}: {}",
                    &ext.name, coredb_name, err
                );
                requeue = true
            }
        }
    }
    if requeue {
        return Err(Action::requeue(Duration::from_secs(10)));
    }
    Ok(current_trunk_install_statuses)
}
