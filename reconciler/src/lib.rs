mod pg_cluster_crd;

use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, Client};
use pg_cluster_crd::PostgresCluster;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub async fn get_all(client: Client, namespace: String) -> Vec<PostgresCluster> {
    let pg_cluster_api: Api<PostgresCluster> = Api::namespaced(client, &namespace);
    let pg_list = pg_cluster_api
        .list(&ListParams::default())
        .await
        .expect("could not get PostgresClusters");
    pg_list.items
}

pub async fn create_or_update(
    client: Client,
    namespace: String,
    deployment: serde_json::Value,
) -> Result<(), Error> {
    let pg_cluster_api: Api<PostgresCluster> = Api::namespaced(client, &namespace);
    let params = PatchParams::apply("reconciler").force();
    let name: String = serde_json::from_value(deployment["metadata"]["name"].clone()).unwrap();
    println!("\nCreating or updating PostgresCluster: {}", name);
    let _o = pg_cluster_api
        .patch(&name, &params, &Patch::Apply(&deployment))
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}

pub async fn delete(client: Client, namespace: String, name: String) -> Result<(), Error> {
    let pg_cluster_api: Api<PostgresCluster> = Api::namespaced(client, &namespace);
    let params = DeleteParams::default();
    println!("\nDeleting PostgresCluster: {}", name);
    let _o = pg_cluster_api
        .delete(&name, &params)
        .await
        .map_err(Error::KubeError);
    Ok(())
}
