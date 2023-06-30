use crate::{apis::coredb_types::CoreDB, Context, Error};
use k8s_openapi::{api::core::v1::Secret, apimachinery::pkg::apis::meta::v1::ObjectMeta, ByteString};
use kube::{
    api::{ListParams, Patch, PatchParams},
    Api, Client, Resource, ResourceExt,
};
use passwords::PasswordGenerator;
use std::{collections::BTreeMap, sync::Arc};
use tracing::debug;

#[derive(Clone, Debug)]
pub struct PrometheusExporterSecretData {
    pub password: String,
}

pub async fn reconcile_secret(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-connection", cdb.name_any());
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let secret_api: Api<Secret> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    // check for existing secret
    let lp = ListParams::default().labels("app=coredb");
    let secrets = secret_api.list(&lp).await.expect("could not get Secrets");

    // if the secret has already been created, return (avoids overwriting password value)
    if !secrets.items.is_empty() {
        for s in &secrets.items {
            if s.name_any() == name {
                debug!("skipping secret creation: secret {} exists", &name);
                return Ok(());
            }
        }
    }

    // generate secret data
    let data = secret_data(cdb, &name, &ns);

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
    Ok(())
}

fn secret_data(cdb: &CoreDB, name: &str, ns: &str) -> BTreeMap<String, ByteString> {
    let mut data = BTreeMap::new();

    // encode and insert user into secret data
    let user = "postgres".to_owned();
    let b64_user = b64_encode(&user);
    data.insert("user".to_owned(), b64_user);

    // encode and insert password into secret data
    let password = generate_password();
    let b64_password = b64_encode(&password);
    data.insert("password".to_owned(), b64_password);

    // encode and insert port into secret data
    let port = cdb.spec.port.to_string();
    let b64_port = b64_encode(&port);
    data.insert("port".to_owned(), b64_port);

    // encode and insert host into secret data
    let host = format!("{}.{}.svc.cluster.local", &name, &ns);
    let b64_host = b64_encode(&host);
    data.insert("host".to_owned(), b64_host);

    // encode and insert uri into secret data
    let uri = format!("postgresql://{}:{}@{}:{}", &user, &password, &host, &port);
    let b64_uri = b64_encode(&uri);
    data.insert("uri".to_owned(), b64_uri);

    data
}

// Set postgres-exporter secret
pub async fn reconcile_postgres_exporter_secret(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Option<PrometheusExporterSecretData>, Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-metrics", cdb.name_any());
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let secret_api: Api<Secret> = Api::namespaced(client.clone(), &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), name.to_string());
    labels.insert("component".to_owned(), "metrics".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    // check for existing secret
    let lp = ListParams::default().labels(format!("app={}", &name).as_str());
    let secrets = secret_api.list(&lp).await.expect("could not get Secrets");

    // if the secret has already been created, return (avoids overwriting password value)
    if !secrets.items.is_empty() {
        for s in &secrets.items {
            if s.name_any() == name {
                debug!("skipping secret creation: secret {} exists", &name);
                let secret_data = fetch_secret_data(client.clone(), name, &ns).await?;
                return Ok(Some(secret_data));
            }
        }
    }

    // generate secret data
    let (data, secret_data) = postgres_exporter_secret_data();

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

fn postgres_exporter_secret_data() -> (BTreeMap<String, ByteString>, PrometheusExporterSecretData) {
    let mut data = BTreeMap::new();

    // encode and insert password into secret data
    let password = generate_password();
    let b64_password = b64_encode(&password);
    data.insert("password".to_owned(), b64_password);

    let secret_data = PrometheusExporterSecretData { password };

    (data, secret_data)
}

// Lookup secret data for postgres-exporter
async fn fetch_secret_data(
    client: Client,
    name: String,
    ns: &str,
) -> Result<PrometheusExporterSecretData, Error> {
    let secret_api: Api<Secret> = Api::namespaced(client, ns);
    let secret_name = format!("{}", name);

    match secret_api.get(&secret_name).await {
        Ok(secret) => {
            if let Some(data_map) = secret.data {
                if let Some(password_bytes) = data_map.get("password") {
                    let password = String::from_utf8(password_bytes.0.clone()).unwrap();
                    let secret_data = PrometheusExporterSecretData { password };
                    Ok(secret_data)
                } else {
                    Err(Error::MissingSecretError(
                        "No password found in secret".to_owned(),
                    ))
                }
            } else {
                Err(Error::MissingSecretError("No data found in secret".to_owned()))
            }
        }
        Err(e) => Err(Error::KubeError(e)),
    }
}

fn b64_encode(string: &str) -> ByteString {
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
