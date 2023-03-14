pub mod coredb_crd;
pub mod errors;
mod ingress_route_tcp_crd;
pub mod types;

use base64::{engine::general_purpose, Engine as _};
use coredb_crd as crd;
use coredb_crd::CoreDB;
use errors::ReconcilerError;
use ingress_route_tcp_crd::IngressRouteTCP;
use k8s_openapi::api::core::v1::{Namespace, Secret};
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::runtime::wait::{await_condition, Condition};
use kube::{Api, Client};
use log::{debug, info};
use serde_json::{from_str, to_string, Value};

pub type Result<T, E = ReconcilerError> = std::result::Result<T, E>;

pub async fn generate_spec(namespace: &str, spec: &crd::CoreDBSpec) -> Value {
    let spec = serde_json::json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "metadata": {
            "name": namespace,
        },
        "spec": spec,
    });
    spec
}

pub async fn create_ing_route_tcp(client: Client, name: &str) -> Result<(), ReconcilerError> {
    let ing_api: Api<IngressRouteTCP> = Api::namespaced(client, name);
    let params = PatchParams::apply("reconciler").force();
    let ing = serde_json::json!({
        "apiVersion": "traefik.containo.us/v1alpha1",
        "kind": "IngressRouteTCP",
        "metadata": {
            "name": format!("{name}"),
            "namespace": format!("{name}"),
        },
        "spec": {
            "entryPoints": ["postgresql"],
            "routes": [
                {
                    "match": format!("HostSNI(`{name}.coredb.io`) || HostSNI(`{name}.coredb-development.com`)"),
                    "services": [
                        {
                            "name": format!("{name}"),
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
        .patch(name, &params, &Patch::Apply(&ing))
        .await
        .map_err(ReconcilerError::KubeError)?;
    Ok(())
}

pub async fn create_metrics_ingress(client: Client, name: &str) -> Result<(), ReconcilerError> {
    let ing_api: Api<Ingress> = Api::namespaced(client, name);
    let params = PatchParams::apply("reconciler").force();
    let ingress = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "Ingress",
        "metadata": {
            "name": format!("{name}"),
            "namespace": format!("{name}"),
        },
        "spec": {
            "ingressClassName": "traefik",
            "rules": [
                {
                    "host": format!("{name}.coredb-development.com"),
                    "http": {
                        "paths": [
                            {
                                "path": "/metrics",
                                "pathType": "Prefix",
                                "backend": {
                                    "service": {
                                        "name": format!("{name}-metrics"),
                                        "port": {
                                            "number": 80,
                                        },
                                    },
                                },
                            },
                        ],
                    },
                },
            ],
        },
    });

    info!("\nCreating or updating Ingress: {}", name);
    let _o = ing_api
        .patch(name, &params, &Patch::Apply(&ingress))
        .await
        .map_err(ReconcilerError::KubeError)?;
    Ok(())
}

pub async fn get_all(client: Client, namespace: &str) -> Vec<CoreDB> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let pg_list = coredb_api
        .list(&ListParams::default())
        .await
        .expect("could not get CoreDBs");
    pg_list.items
}

pub async fn get_one(client: Client, namespace: &str) -> Result<CoreDB, ReconcilerError> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let pg_instance = coredb_api.get(namespace).await?;
    debug!("Namespace: {}, CoreDB: {:?}", namespace, pg_instance);
    Ok(pg_instance)
}

// returns CoreDB when status is present, otherwise returns an error
pub async fn get_coredb_status(
    client: Client,
    namespace: &str,
) -> Result<crd::CoreDB, ReconcilerError> {
    let coredb = get_one(client, namespace).await?;

    if coredb.status.is_none() {
        Err(ReconcilerError::NoStatusReported)
    } else {
        Ok(coredb)
    }
}

pub async fn create_or_update(
    client: Client,
    namespace: &str,
    deployment: serde_json::Value,
) -> Result<(), ReconcilerError> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let params = PatchParams::apply("reconciler").force();
    let name: String = serde_json::from_value(deployment["metadata"]["name"].clone()).unwrap();
    info!("\nCreating or updating CoreDB: {}", name);
    let _ = coredb_api
        .patch(&name, &params, &Patch::Apply(&deployment))
        .await
        .map_err(ReconcilerError::KubeError)?;
    Ok(())
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), ReconcilerError> {
    let coredb_api: Api<CoreDB> = Api::namespaced(client, namespace);
    let params = DeleteParams::default();
    info!("\nDeleting CoreDB: {}", name);
    let _o = coredb_api
        .delete(name, &params)
        .await
        .map_err(ReconcilerError::KubeError);
    Ok(())
}

pub async fn create_namespace(client: Client, name: &str) -> Result<(), ReconcilerError> {
    let ns_api: Api<Namespace> = Api::all(client);
    let params = PatchParams::apply("reconciler").force();
    let ns = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": {
            "name": format!("{name}"),
        }
    });
    info!("\nCreating namespace {} if it does not exist", name);
    let _o = ns_api
        .patch(name, &params, &Patch::Apply(&ns))
        .await
        .map_err(ReconcilerError::KubeError)?;
    Ok(())
}

pub async fn delete_namespace(client: Client, name: &str) -> Result<(), ReconcilerError> {
    let ns_api: Api<Namespace> = Api::all(client);
    let params = DeleteParams::default();
    info!("\nDeleting namespace: {}", name);
    let _ = ns_api
        .delete(name, &params)
        .await
        .map_err(ReconcilerError::KubeError);
    Ok(())
}

// remove after COR-166
#[allow(unused_variables)]
pub async fn get_pg_conn(client: Client, name: &str) -> Result<String, ReconcilerError> {
    // read secret <name>-connection
    let secret_name = format!("{name}-connection");

    let secret_api: Api<Secret> = Api::namespaced(client, name);

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

    let host = format!("{name}.coredb-development.com");
    let connection_string = format!("postgresql://{user}:{password}@{host}:5432");

    Ok(connection_string)
}

#[allow(dead_code)] // remove after COR-166
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
