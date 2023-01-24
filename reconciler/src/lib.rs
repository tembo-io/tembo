mod ingress_route_tcp_crd;
mod pg_cluster_crd;

use ingress_route_tcp_crd::IngressRouteTCP;
use k8s_openapi::api::core::v1::{Namespace, Secret};
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::runtime::wait::{await_condition, Condition};
use kube::{Api, Client};
use log::info;
use pg_cluster_crd::PostgresCluster;
use serde_json::{from_str, to_string, Value};
use std::fmt::Debug;
use thiserror::Error;

use base64::{engine::general_purpose, Engine as _};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub async fn generate_spec(body: Value) -> Value {
    let name: String = serde_json::from_value(body["resource_name"].clone()).unwrap();
    let cpu: String = serde_json::from_value(body["cpu"].clone()).unwrap();
    let memory: String = serde_json::from_value(body["memory"].clone()).unwrap();
    let storage: String = serde_json::from_value(body["storage"].clone()).unwrap();

    let spec = serde_json::json!({
        "apiVersion": "postgres-operator.crunchydata.com/v1beta1",
        "kind": "PostgresCluster",
        "metadata": {
            "name": format!("{}", name),
        },
        "spec": {
            "image": "registry.developers.crunchydata.com/crunchydata/crunchy-postgres:ubi8-14.6-2".to_owned(),
            "postgresVersion": 14,
            "instances": [
                {
                    "name": "instance1",
                    "dataVolumeClaimSpec": {
                        "accessModes": ["ReadWriteOnce"],
                        "resources": {"requests": {"storage": format!("{}", storage)}},
                    },
                    "resources": {
                        "limits": {
                            "cpu": format!("{}", cpu),
                            "memory": format!("{}", memory),
                        },
                        "requests": {
                            "cpu": format!("{}", cpu),
                            "memory": format!("{}", memory),
                        },
                    },
                },
            ],
            "backups": {
                "pgbackrest": {
                    "image": "registry.developers.crunchydata.com/crunchydata/crunchy-pgbackrest:ubi8-2.41-2",
                    "repos": [
                        {
                            "name": "repo1",
                            "volume": {
                                "volumeClaimSpec": {
                                    "accessModes": ["ReadWriteOnce"],
                                    "resources": {"requests": {"storage": "1Gi"}},
                                },
                            },
                        },
                    ],
                }
            },
        },
    });
    spec
}

pub async fn create_ing_route_tcp(client: Client, name: String) -> Result<(), Error> {
    let ing_api: Api<IngressRouteTCP> = Api::namespaced(client, &name);
    let params = PatchParams::apply("reconciler").force();
    let ing = serde_json::json!({
        "apiVersion": "traefik.containo.us/v1alpha1",
        "kind": "IngressRouteTCP",
        "metadata": {
            "name": format!("{}", name),
            "namespace": format!("{}", name),
        },
        "spec": {
            "entryPoints": ["postgresql"],
            "routes": [
                {
                    "match": format!("HostSNI(`{}.coredb.io`) || HostSNI(`{}.coredb-development.com`)", name, name),
                    "services": [
                        {
                            "name": format!("{}-primary", name),
                            "port": 5432,
                        },
                    ],
                },
            ],
            "tls": {
                "passthrough": true,
            },
        },
    });
    info!("\nCreating or updating IngressRouteTCP: {}", name);
    let _o = ing_api
        .patch(&name, &params, &Patch::Apply(&ing))
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}

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
    info!("\nCreating or updating PostgresCluster: {}", name);
    let _o = pg_cluster_api
        .patch(&name, &params, &Patch::Apply(&deployment))
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}

pub async fn delete(client: Client, namespace: String, name: String) -> Result<(), Error> {
    let pg_cluster_api: Api<PostgresCluster> = Api::namespaced(client, &namespace);
    let params = DeleteParams::default();
    info!("\nDeleting PostgresCluster: {}", name);
    let _o = pg_cluster_api
        .delete(&name, &params)
        .await
        .map_err(Error::KubeError);
    Ok(())
}

pub async fn create_namespace(client: Client, name: String) -> Result<(), Error> {
    let ns_api: Api<Namespace> = Api::all(client);
    let params = PatchParams::apply("reconciler").force();
    let ns = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": {
            "name": format!("{}", name),
        }
    });
    info!("\nCreating namespace {} if it does not exist", name);
    let _o = ns_api
        .patch(&name, &params, &Patch::Apply(&ns))
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}

pub async fn delete_namespace(client: Client, name: String) -> Result<(), Error> {
    let ns_api: Api<Namespace> = Api::all(client);
    let params = DeleteParams::default();
    info!("\nDeleting namespace: {}", name);
    let _o = ns_api
        .delete(&name, &params)
        .await
        .map_err(Error::KubeError);
    Ok(())
}

pub async fn get_pg_conn(client: Client, name: String) -> Result<String, Error> {
    // read secret <name>-pguser-name
    let secret_name = format!("{}-pguser-{}", name, name);
    let secret_api: Api<Secret> = Api::namespaced(client, &name.clone());

    // wait for secret to exist
    let establish = await_condition(secret_api.clone(), &secret_name, wait_for_secret());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(90), establish).await;

    let secret = secret_api
        .get(secret_name.as_str())
        .await
        .expect("error getting Secret");

    let data = secret.data.unwrap();

    // TODO(ianstanton) There has to be a better way to do this
    let user_data = data.get("user").unwrap();
    let byte_user = to_string(user_data).unwrap();
    let string_user: String = from_str(&byte_user).unwrap();

    let pw_data = data.get("password").unwrap();
    let byte_pw = to_string(pw_data).unwrap();
    let string_pw: String = from_str(&byte_pw).unwrap();

    let user = b64_decode(&string_user);
    let password = b64_decode(&string_pw);

    let host = format!("{}.coredb-development.com", name);
    let connection_string = format!("postgresql://{}:{}@{}:5432", user, password, host);

    Ok(connection_string)
}

fn b64_decode(b64_encoded: &str) -> String {
    let bytes = general_purpose::STANDARD.decode(b64_encoded).unwrap();
    std::str::from_utf8(&bytes).unwrap().to_owned()
}

// TODO(ianstanton) This is a hack for now. We need to find a more 'official' way of checking for
//  existing resources in the cluster.
pub fn wait_for_secret() -> impl Condition<Secret> {
    |obj: Option<&Secret>| {
        if let Some(secret) = &obj {
            if let Some(t) = &secret.type_ {
                return t == "Opaque";
            }
        }
        false
    }
}

#[test]
fn test_b64_decode_string() {
    let encoded = "SGVsbG8sIFdvcmxkIQ==";
    let decoded = b64_decode(encoded);
    assert_eq!(decoded, "Hello, World!");

    let encoded = "ZnJpZGF5";
    let decoded = b64_decode(encoded);
    assert_eq!(decoded, "friday");
}

#[test]
fn test_b64_decode_empty_string() {
    let encoded = "";
    let decoded = b64_decode(encoded);
    assert_eq!(decoded, "");
}
