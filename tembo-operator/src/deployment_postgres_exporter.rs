use crate::{
    apis::coredb_types::CoreDB,
    defaults::default_postgres_exporter_image,
    postgres_exporter::{EXPORTER_CONFIGMAP_PREFIX, EXPORTER_VOLUME, QUERIES_YAML},
    rbac::reconcile_rbac,
    Context, Error, Result,
};
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            ConfigMapVolumeSource, Container, ContainerPort, EnvVar, EnvVarSource, HTTPGetAction, PodSpec,
            PodTemplateSpec, Probe, SecretKeySelector, SecurityContext, Volume, VolumeMount,
        },
        rbac::v1::PolicyRule,
    },
    apimachinery::pkg::{apis::meta::v1::LabelSelector, util::intstr::IntOrString},
};
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams, ResourceExt},
    Resource,
};
use std::{collections::BTreeMap, sync::Arc};


const PROM_CFG_DIR: &str = "/prometheus";

pub async fn reconcile_prometheus_exporter_deployment(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let coredb_name = cdb.metadata.name.clone().expect("should always have a name");
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-metrics", cdb.name_any());
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let deployment_api: Api<Deployment> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "postgres-exporter".to_string());
    labels.insert("component".to_owned(), "metrics".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    // Format the postgres-exporter connection URI
    // Check if cnpg is enabled, if so then set the URI to the cnpg service
    // Otherwise, use the old coredb service
    let psql_uri: String = format!("{}-rw.{}.svc.cluster.local:5432/postgres", cdb.name_any(), ns);

    // reconcile rbac(service account, role, role binding) for the postgres-exporter
    let rbac = reconcile_rbac(
        cdb,
        ctx.clone(),
        Some("metrics"),
        create_policy_rules(name.clone()).await,
    )
    .await?;

    // Generate the ObjectMeta for the Deployment
    let deployment_metadata = ObjectMeta {
        name: Some(name.to_owned()),
        namespace: Some(ns.to_owned()),
        labels: Some(labels.clone()),
        owner_references: Some(vec![oref]),
        ..ObjectMeta::default()
    };

    // 0 replicas on deployment when stopping
    // 1 replica in all other cases
    let replicas = match cdb.spec.stop {
        true => 0,
        false => 1,
    };

    // Generate the Probe for the Container
    let readiness_probe = Probe {
        http_get: Some(HTTPGetAction {
            path: Some("/metrics".to_string()),
            port: IntOrString::String("metrics".to_string()),
            ..HTTPGetAction::default()
        }),
        initial_delay_seconds: Some(3),
        ..Probe::default()
    };

    // Generate ContainerPort for the Container
    let container_port = vec![ContainerPort {
        container_port: 9187,
        name: Some("metrics".to_string()),
        protocol: Some("TCP".to_string()),
        ..ContainerPort::default()
    }];

    // Generate SecurityContext for the Container
    let security_context = SecurityContext {
        run_as_user: Some(65534),
        allow_privilege_escalation: Some(false),
        ..SecurityContext::default()
    };

    // Generate EnvVar for the Container
    let env_vars = vec![
        EnvVar {
            name: "DATA_SOURCE_URI".to_string(),
            value: Some(psql_uri.clone()),
            ..EnvVar::default()
        },
        EnvVar {
            name: "DATA_SOURCE_USER".to_string(),
            value: Some("postgres_exporter".to_string()),
            ..EnvVar::default()
        },
        // Set EnvVar from a secret
        EnvVar {
            name: "DATA_SOURCE_PASS".to_string(),
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    key: "password".to_string(),
                    name: Some(name.to_owned()),
                    optional: Some(false),
                }),
                ..EnvVarSource::default()
            }),
            ..EnvVar::default()
        },
        EnvVar {
            name: "PG_EXPORTER_EXTEND_QUERY_PATH".to_string(),
            value: Some(format!("{PROM_CFG_DIR}/{QUERIES_YAML}")),
            ..EnvVar::default()
        },
    ];

    // Generate VolumeMounts for the Container
    let exporter_vol_mounts = match cdb.spec.metrics {
        None => {
            vec![]
        }
        Some(_) => {
            vec![VolumeMount {
                name: EXPORTER_VOLUME.to_owned(),
                mount_path: PROM_CFG_DIR.to_string(),
                ..VolumeMount::default()
            }]
        }
    };

    // Generate Volumes for the PodSpec
    let exporter_volumes = match cdb.spec.metrics {
        None => {
            vec![]
        }
        Some(_) => {
            vec![Volume {
                config_map: Some(ConfigMapVolumeSource {
                    name: Some(format!("{}{}", EXPORTER_CONFIGMAP_PREFIX.to_owned(), coredb_name)),
                    ..ConfigMapVolumeSource::default()
                }),
                name: EXPORTER_VOLUME.to_owned(),
                ..Volume::default()
            }]
        }
    };

    // Generate the PodSpec for the PodTemplateSpec
    let pod_spec = PodSpec {
        containers: vec![Container {
            args: Some(vec!["--auto-discover-databases".to_string()]),
            env: Some(env_vars),
            image: Some(get_exporter_image(&cdb.clone())),
            name: "postgres-exporter".to_string(),
            ports: Some(container_port),
            readiness_probe: Some(readiness_probe),
            security_context: Some(security_context),
            volume_mounts: Some(exporter_vol_mounts),
            ..Container::default()
        }],
        service_account: rbac.service_account.metadata.name.clone(),
        service_account_name: rbac.service_account.metadata.name.clone(),
        volumes: Some(exporter_volumes),
        ..PodSpec::default()
    };

    // Generate the PodTemplateSpec for the DeploymentSpec
    let pod_template_spec = PodTemplateSpec {
        metadata: Some(deployment_metadata.clone()),
        spec: Some(pod_spec),
    };

    // Generate the DeploymentSpec for the Deployment
    let deployment_spec = DeploymentSpec {
        replicas: Some(replicas),
        selector: LabelSelector {
            match_labels: Some(labels.clone()),
            ..LabelSelector::default()
        },
        template: pod_template_spec,
        ..DeploymentSpec::default()
    };

    // Generate the Deployment for Prometheus Exporter
    let deployment = Deployment {
        metadata: deployment_metadata,
        spec: Some(deployment_spec),
        ..Deployment::default()
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = deployment_api
        .patch(&name, &ps, &Patch::Apply(&deployment))
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}

// Generate the PolicyRules for the Role
async fn create_policy_rules(name: String) -> Vec<PolicyRule> {
    vec![
        // This policy allows get, watch access to a secret in the namespace
        PolicyRule {
            api_groups: Some(vec!["".to_owned()]),
            resource_names: Some(vec![format!("{}", name)]),
            resources: Some(vec!["secrets".to_owned()]),
            verbs: vec!["get".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
    ]
}

fn get_exporter_image(cdb: &CoreDB) -> String {
    // Check if cdb.spec.postgresExporterImage is set
    // If so, use that image; otherwise, use the default
    // image from default_postgres_exporter_image() function
    if cdb.spec.postgresExporterImage.is_empty() {
        default_postgres_exporter_image()
    } else {
        cdb.spec.postgresExporterImage.clone()
    }
}
