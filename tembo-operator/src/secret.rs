use crate::{apis::coredb_types::CoreDB, Context, Error};

use base64::{engine::general_purpose, Engine as _};
use k8s_openapi::{
    api::core::v1::Secret, apimachinery::pkg::apis::meta::v1::ObjectMeta, ByteString,
};
use kube::{
    api::{ListParams, Patch, PatchParams},
    runtime::controller::Action,
    Api, Resource, ResourceExt,
};
use passwords::PasswordGenerator;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use std::{collections::BTreeMap, sync::Arc};
use tokio::time::Duration;
use tracing::{debug, error};

#[derive(Clone)]
pub struct RolePassword {
    pub password: String,
}

pub async fn reconcile_secret(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-connection", cdb.name_any());
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let secret_api: Api<Secret> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    // check for existing secret
    let lp = ListParams::default()
        .labels(format!("app=coredb,coredb.io/name={}", cdb.name_any()).as_str());
    let secrets = match secret_api.list(&lp).await {
        Ok(secrets) => secrets,
        Err(e) => {
            error!("Failed to list secrets: {}", e);
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };

    // If the secret is already created, re-use the password
    let password = match secrets.items.is_empty() {
        true => generate_password(),
        false => {
            let secret_data = secrets.items[0]
                .data
                .clone()
                .expect("Expect to always have 'data' block in a kubernetes secret");
            let password_bytes = secret_data
                .get("password")
                .expect("could not find password");
            let password_encoded = serde_json::to_string(password_bytes)
                .expect("Expected to be able decode from byte string to base64-encoded string");
            let password_encoded = password_encoded.as_str();
            let password_encoded = password_encoded.trim_matches('"');
            let bytes = general_purpose::STANDARD
                .decode(password_encoded)
                .expect("Expect to always be able to base64 decode a kubernetes secret value");

            String::from_utf8(bytes)
                .expect("Expect to always be able to convert a kubernetes secret value to a string")
        }
    };

    let data = secret_data(cdb, &ns, password);

    let secret: Secret = Secret {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(ns.to_owned()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref]),
            ..ObjectMeta::default()
        },
        data: Some(data),
        ..Secret::default()
    };

    let ps = PatchParams::apply("cntrlr").force();
    let patch_status = Patch::Apply(&secret);
    match secret_api.patch(&name, &ps, &patch_status).await {
        Ok(_) => {
            debug!("Successfully updated secret for instance: {}", name);
            Ok(())
        }
        Err(e) => {
            error!("Error updating secret for {}: {:?}", name, e);
            Err(Action::requeue(Duration::from_secs(10)))
        }
    }
}

fn secret_data(cdb: &CoreDB, ns: &str, password: String) -> BTreeMap<String, ByteString> {
    let mut data = BTreeMap::new();

    // encode and insert user into secret data
    let user = "postgres".to_owned();
    let b64_user = b64_encode(&user);
    // Add as both 'user' and 'username'
    data.insert("user".to_owned(), b64_user.clone());
    data.insert("username".to_owned(), b64_user);

    // encode and insert password into secret data
    let b64_password = b64_encode(&password);
    data.insert("password".to_owned(), b64_password);

    // encode and insert port into secret data
    let port = cdb.spec.port.to_string();
    let b64_port = b64_encode(&port);
    data.insert("port".to_owned(), b64_port);

    // read only host
    let r_host = format!("{}-r.{}.svc.cluster.local", &cdb.name_any(), &ns);
    // read/write host
    let rw_host = format!("{}-rw.{}.svc.cluster.local", &cdb.name_any(), &ns);
    // read only host
    let ro_host: String = format!("{}-ro.{}.svc.cluster.local", &cdb.name_any(), &ns);
    // pooler host
    let pooler_host = format!("{}-pooler.{}.svc.cluster.local", &cdb.name_any(), &ns);

    // encode and insert host into secret data
    let b64_host = b64_encode(&r_host);
    data.insert("host".to_owned(), b64_host);

    // encode and insert uri into secret data
    let uri = format!("postgresql://{}:{}@{}:{}", &user, &password, &r_host, &port);
    let b64_uri = b64_encode(&uri);
    data.insert("r_uri".to_owned(), b64_uri);

    // encode and insert read-write uri into secret data
    let rwuri = format!(
        "postgresql://{}:{}@{}:{}",
        &user, &password, &rw_host, &port
    );
    let b64_rwuri = b64_encode(&rwuri);
    data.insert("rw_uri".to_owned(), b64_rwuri);

    // encode and insert read-only uri into secret data
    let rouri = format!(
        "postgresql://{}:{}@{}:{}",
        &user, &password, &ro_host, &port
    );
    let b64_rouri = b64_encode(&rouri);
    data.insert("ro_uri".to_owned(), b64_rouri);

    // encode and insert pooler uri into secret data
    if cdb.spec.connectionPooler.enabled {
        let pooler_uri = format!(
            "postgresql://{}:{}@{}:{}",
            &user, &password, &pooler_host, &port
        );
        let b64_pooler_uri = b64_encode(&pooler_uri);
        data.insert("pooler_uri".to_owned(), b64_pooler_uri);
    }

    data
}

// Set postgres-exporter secret
pub async fn reconcile_postgres_role_secret(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    role_name: &str,
    secret_name: &str,
) -> Result<Option<RolePassword>, Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = secret_name.to_string();
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let secret_api: Api<Secret> = Api::namespaced(client.clone(), &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("role".to_owned(), role_name.to_string());
    labels.insert("tembo.io/name".to_owned(), cdb.name_any());

    // Get secret by name
    if secret_api.get(secret_name).await.is_ok() {
        debug!("skipping secret creation: secret {} exists", &name);
        let secret_api: Api<Secret> = Api::namespaced(client.clone(), &ns);
        let password = match fetch_all_decoded_data_from_secret(secret_api, name)
            .await?
            .get("password")
        {
            Some(password) => password.to_owned(),
            None => {
                return Err(Error::MissingSecretError(
                    "Did not find key 'password' in secret".to_owned(),
                ))
            }
        };
        let secret_data = RolePassword { password };
        return Ok(Some(secret_data));
    };

    // generate secret data
    let (data, secret_data) = generate_role_secret_data(role_name);

    let secret: Secret = Secret {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(ns.to_owned()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![oref]),
            ..ObjectMeta::default()
        },
        data: Some(data),
        ..Secret::default()
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = secret_api
        .patch(&name, &ps, &Patch::Apply(&secret))
        .await
        .map_err(Error::KubeError)?;
    Ok(Some(secret_data))
}

fn generate_role_secret_data(role_name: &str) -> (BTreeMap<String, ByteString>, RolePassword) {
    let mut data = BTreeMap::new();

    // encode and insert password into secret data
    let password = generate_password();
    let b64_password = b64_encode(&password);
    data.insert("password".to_owned(), b64_password);
    data.insert("username".to_owned(), b64_encode(role_name));

    let secret_data = RolePassword { password };

    (data, secret_data)
}

// Lookup secret data for postgres-exporter
pub async fn fetch_all_decoded_data_from_secret(
    secrets_api: Api<Secret>,
    name: String,
) -> Result<BTreeMap<String, String>, Error> {
    let secret_name = name.to_string();

    match secrets_api.get(&secret_name).await {
        Ok(secret) => {
            if let Some(data_map) = secret.data {
                let mut decoded_data = BTreeMap::new();

                for (key, secret_bytes) in data_map {
                    match String::from_utf8(secret_bytes.0.clone()) {
                        Ok(decoded_string) => {
                            decoded_data.insert(key, decoded_string);
                        }
                        Err(_) => {
                            return Err(Error::MissingSecretError(format!(
                                "Failed to decode data for key {}",
                                key
                            )));
                        }
                    }
                }

                Ok(decoded_data)
            } else {
                Err(Error::MissingSecretError(
                    "No data found in secret".to_owned(),
                ))
            }
        }
        Err(e) => Err(Error::KubeError(e)),
    }
}

pub fn b64_encode(string: &str) -> ByteString {
    let bytes_vec = string.as_bytes().to_vec();
    ByteString(bytes_vec)
}

fn generate_password() -> String {
    let pg = PasswordGenerator {
        length: 16,
        numbers: true,
        lowercase_letters: true,
        uppercase_letters: true,
        symbols: false,
        spaces: false,
        exclude_similar_characters: false,
        strict: true,
    };
    pg.generate_one().unwrap()
}
