use crate::{Context, CoreDB, Error};
use k8s_openapi::{api::core::v1::Secret, apimachinery::pkg::apis::meta::v1::ObjectMeta, ByteString};
use kube::{
    api::{Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use passwords::PasswordGenerator;
use std::{collections::BTreeMap, sync::Arc};

pub async fn reconcile_secret(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = cdb.name_any();
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let mut data: BTreeMap<String, ByteString> = BTreeMap::new();
    let secret_api: Api<Secret> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "coredb".to_owned());

    // encode and insert user into secret data
    let user = "postgres".to_owned();
    let b64_user = b64_encode(user);
    data.insert("user".to_owned(), b64_user);

    // encode and insert password into secret data
    let password = generate_password();
    let b64_password = b64_encode(password);
    data.insert("password".to_owned(), b64_password);

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

fn b64_encode(string: String) -> ByteString {
    let bytes_vec = string.as_bytes().to_vec();
    let byte_string = ByteString(bytes_vec);
    byte_string
}

fn generate_password() -> String {
    let pg = PasswordGenerator {
        length: 8,
        numbers: true,
        lowercase_letters: true,
        uppercase_letters: true,
        symbols: true,
        spaces: false,
        exclude_similar_characters: false,
        strict: true,
    };
    let password = pg.generate_one().unwrap();
    password
}
