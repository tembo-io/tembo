use crate::{
    apis::coredb_types::CoreDB,
    certmanager::certificates::Certificate,
    secret::{b64_encode, fetch_all_decoded_data_from_secret},
};
use k8s_openapi::api::core::v1::Secret;
use kube::{
    api::{Api, Patch, PatchParams},
    runtime::controller::Action,
    Client,
};
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, error};

const POSTGRES_CA_SECRET_NAME: &str = "postgres-ca-secret";
const POSTGRES_CA_SECRET_CERT_KEY_NAME: &str = "ca.crt";
const POSTGRES_CERTIFICATE_ISSUER_NAME: &str = "postgres-server-issuer";

pub async fn reconcile_certificates(
    client: Client,
    coredb: &CoreDB,
    namespace: &str,
) -> Result<(), Action> {
    match std::env::var("USE_SHARED_CA") {
        Ok(_) => {}
        Err(_) => {
            debug!("USE_SHARED_CA not set, skipping certificate reconciliation");
            return Ok(());
        }
    }

    let coredb_name = coredb.metadata.name.as_ref().unwrap();
    let secrets_api_cert_manager_namespace: Api<Secret> =
        Api::namespaced(client.clone(), "cert-manager");
    let secrets_api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let certificates_api: Api<Certificate> = Api::namespaced(client, namespace);

    let decoded_ca_cert = match fetch_all_decoded_data_from_secret(
        secrets_api_cert_manager_namespace,
        POSTGRES_CA_SECRET_NAME.to_string(),
    )
    .await
    {
        Ok(decoded_ca_cert) => match decoded_ca_cert.get(POSTGRES_CA_SECRET_CERT_KEY_NAME) {
            None => {
                error!(
                    "Failed to fetch CA certificate from cert-manager namespace: {}",
                    POSTGRES_CA_SECRET_CERT_KEY_NAME
                );
                return Err(Action::requeue(Duration::from_secs(300)));
            }
            Some(ca) => ca.to_owned(),
        },
        Err(e) => {
            error!(
                "Failed to fetch CA certificate from cert-manager namespace: {}",
                e
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };

    // Create a new secret in the target namespace with the fetched value
    let secret_name = format!("{}-ca1", coredb_name);
    let new_secret = json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": secret_name,
            "namespace": namespace,
        },
        "data": {
            "ca.crt": b64_encode(&decoded_ca_cert)
        },
        "type": "Opaque"
    });
    let ps = PatchParams::apply("cntrlr").force();
    let _o = match secrets_api
        .patch(&secret_name, &ps, &Patch::Apply(&new_secret))
        .await
    {
        Ok(_secret) => _secret,
        Err(e) => {
            error!(
                "Failed to apply CA certificate secret from cert-manager namespace to namespace {}, {}",
                namespace, e
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };

    let common_name = format!("{}-rw", coredb_name);
    let mut dns_names = vec![
        format!("{}-rw", coredb_name),
        format!("{}-rw.{}", coredb_name, namespace),
        format!("{}-rw.{}.svc", coredb_name, namespace),
        format!("{}-rw.{}.svc.cluster.local", coredb_name, namespace),
        format!("{}-r", coredb_name),
        format!("{}-r.{}", coredb_name, namespace),
        format!("{}-r.{}.svc", coredb_name, namespace),
        format!("{}-r.{}.svc.cluster.local", coredb_name, namespace),
        format!("{}-ro", coredb_name),
        format!("{}-ro.{}", coredb_name, namespace),
        format!("{}-ro.{}.svc", coredb_name, namespace),
        format!("{}-ro.{}.svc.cluster.local", coredb_name, namespace),
        format!("{}-pooler", coredb_name),
        format!("{}-pooler.{}", coredb_name, namespace),
        format!("{}-pooler.{}.svc", coredb_name, namespace),
        format!("{}-pooler.{}.svc.cluster.local", coredb_name, namespace),
    ];
    match std::env::var("DATA_PLANE_BASEDOMAIN") {
        Ok(basedomain) => {
            let extra_domain_name = format!("{}.{}", coredb_name, basedomain);
            let extra_pooler_domain_name = format!("{}-pooler.{}", coredb_name, basedomain);
            let extra_ro_domain_name = format!("{}-ro.{}", coredb_name, basedomain);
            dns_names.push(extra_domain_name);
            dns_names.push(extra_pooler_domain_name);
            dns_names.push(extra_ro_domain_name);
        }
        Err(_) => {
            debug!("DATA_PLANE_BASEDOMAIN not set, not adding custom DNS name");
        }
    };

    // Create the first Certificate
    let server_certificate = json!({
        "apiVersion": "cert-manager.io/v1",
        "kind": "Certificate",
        "metadata": {
            "name": format!("{}-server1", coredb_name),
            "namespace": namespace,
        },
        "spec": {
            "secretName": format!("{}-server1", coredb_name),
            "usages": ["server auth"],
            "dnsNames": dns_names,
            "commonName": common_name,
            "issuerRef": {
                "name": POSTGRES_CERTIFICATE_ISSUER_NAME,
                "kind": "ClusterIssuer",
                "group": "cert-manager.io"
            }
        }
    });

    apply_certificate(namespace, &certificates_api, server_certificate).await?;

    // Create the second Certificate
    let replication_certificate = json!({
        "apiVersion": "cert-manager.io/v1",
        "kind": "Certificate",
        "metadata": {
            "name": format!("{}-replication1", coredb_name),
            "namespace": namespace,
        },
        "spec": {
            "secretName": format!("{}-replication1", coredb_name),
            "commonName": "streaming_replica",
            "issuerRef": {
                "name": POSTGRES_CERTIFICATE_ISSUER_NAME,
                "kind": "ClusterIssuer",
                "group": "cert-manager.io"
            }
        }
    });

    apply_certificate(namespace, &certificates_api, replication_certificate).await?;

    if coredb.spec.connectionPooler.enabled {
        // Create the third Certificate
        let pooler_certificate = json!({
            "apiVersion": "cert-manager.io/v1",
            "kind": "Certificate",
            "metadata": {
                "name": format!("{}-pooler", coredb_name),
                "namespace": namespace,
            },
            "spec": {
                "secretName": format!("{}-pooler", coredb_name),
                "commonName": "cnpg_pooler_pgbouncer".to_string(),
                "issuerRef": {
                    "name": POSTGRES_CERTIFICATE_ISSUER_NAME,
                    "kind": "ClusterIssuer",
                    "group": "cert-manager.io",
                "usages": ["client auth"]
                }
            }
        });

        apply_certificate(namespace, &certificates_api, pooler_certificate).await?;
    }

    Ok(())
}

async fn apply_certificate(
    namespace: &str,
    cert_api: &Api<Certificate>,
    cert_value: Value,
) -> Result<(), Action> {
    let certificate: Certificate = match serde_json::from_value(cert_value) {
        Ok(cert) => cert,
        Err(_) => {
            error!(
                "Failed to deserialize Certificate in namespace {}",
                namespace
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    let name = certificate.metadata.name.as_ref().ok_or_else(|| {
        error!(
            "Certificate name is empty in namespace: {}.",
            namespace.to_string()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let params: PatchParams = PatchParams::apply("conductor").force();
    debug!("\nApplying Certificate {} in namespace {}", name, namespace);
    let _o: Certificate = match cert_api
        .patch(name, &params, &Patch::Apply(&certificate))
        .await
    {
        Ok(cert) => cert,
        Err(e) => {
            error!(
                "Failed to create Certificate {} in namespace {}. {:?}",
                name, namespace, e
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    Ok(())
}
