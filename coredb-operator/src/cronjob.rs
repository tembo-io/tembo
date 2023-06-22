use crate::{apis::coredb_types::CoreDB, Context, Error};
use k8s_openapi::{
    api::{
        batch::v1::{CronJob, CronJobSpec, JobSpec, JobTemplateSpec},
        core::v1::{Container, PodSpec, PodTemplateSpec, ServiceAccount},
        rbac::v1::{PolicyRule, Role, RoleBinding, RoleRef, Subject},
    },
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    api::{Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc};

pub async fn reconcile_cronjob(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-daily", cdb.name_any());
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let cj_api: Api<CronJob> = Api::namespaced(client, &ns);
    let oref = cdb.controller_owner_ref(&()).unwrap();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    // create service account for cronjob
    let sa = reconcile_service_account(cdb, ctx.clone()).await?;
    // create role for cronjob
    let role = reconcile_role(cdb, ctx.clone()).await?;
    // create role binding for cronjob
    reconcile_role_binding(cdb, ctx.clone(), &sa, role).await?;

    // reconcile cronjob
    let cronjob_metadata = ObjectMeta {
        name: Some(name.to_owned()),
        namespace: Some(ns.to_owned()),
        labels: Some(labels.clone()),
        owner_references: Some(vec![oref]),
        ..ObjectMeta::default()
    };

    let sa_name = sa.metadata.name;

    // create spec for cronjob
    let cj_spec = CronJobSpec {
    schedule: cdb.spec.backup.schedule.as_ref().unwrap().to_string(),
    successful_jobs_history_limit: Some(5),
    job_template: JobTemplateSpec {
        spec: Some(JobSpec {
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    service_account_name: sa_name,
                    containers: vec![Container {
                        name: "full-backup".to_string(),
                        image: Some("quay.io/coredb/kubectl:1.25".to_string()),
                        command: Some(vec![
                            "sh".to_string(),
                            "-c".to_string(),
                            format!(
                                "kubectl exec -it {}-0 -- /bin/sh -c \"/usr/bin/wal-g backup-push /var/lib/postgresql/data --full --verify && /usr/bin/wal-g delete retain {} --confirm\"",
                                cdb.name_any(), cdb.spec.backup.retentionPolicy.as_ref().unwrap()
                            ),
                        ]),
                        ..Container::default()
                    }],
                    restart_policy: Some("OnFailure".to_string()),
                    ..PodSpec::default()
                }),
                ..PodTemplateSpec::default()
            },
            ..JobSpec::default()
        }),
        ..JobTemplateSpec::default()
    },
    ..CronJobSpec::default()
    };

    // now create the cronjob
    let cj = CronJob {
        metadata: cronjob_metadata,
        spec: Some(cj_spec),
        ..CronJob::default()
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = cj_api
        .patch(&name, &ps, &Patch::Apply(&cj))
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}

// reconcile a kubernetes role
async fn reconcile_service_account(cdb: &CoreDB, ctx: Arc<Context>) -> Result<ServiceAccount, Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-backup", cdb.name_any());
    let sa_api: Api<ServiceAccount> = Api::namespaced(client.clone(), &ns);

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    let mut sa_metadata = ObjectMeta {
        name: Some(name.to_owned()),
        namespace: Some(ns.to_owned()),
        labels: Some(labels.clone()),
        ..ObjectMeta::default()
    };

    if let Some(ref template_metadata) = cdb.spec.serviceAccountTemplate.metadata {
        if let Some(ref annotations) = template_metadata.annotations {
            sa_metadata.annotations = Some(annotations.clone());
        }
    }

    let sa = ServiceAccount {
        metadata: sa_metadata,
        ..ServiceAccount::default()
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = sa_api
        .patch(&name, &ps, &Patch::Apply(&sa))
        .await
        .map_err(Error::KubeError)?;

    Ok(sa)
}

// reconcile a kubernetes role
async fn reconcile_role(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Role, Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-backup", cdb.name_any());
    let role_api: Api<Role> = Api::namespaced(client.clone(), &ns);

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    let rules = create_policy_rules();

    let role = Role {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(ns.to_owned()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        rules: Some(rules.await),
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = role_api
        .patch(&name, &ps, &Patch::Apply(&role))
        .await
        .map_err(Error::KubeError)?;

    Ok(role)
}

async fn reconcile_role_binding(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    sa: &ServiceAccount,
    role: Role,
) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-backup", cdb.name_any());
    let role_binding_api: Api<RoleBinding> = Api::namespaced(client.clone(), &ns);
    let sa_name = sa.name_any();
    let role_name = role.name_any();

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    let role_ref = RoleRef {
        api_group: "rbac.authorization.k8s.io".to_string(),
        kind: "Role".to_string(),
        name: role_name.to_string(),
    };

    let subject = Subject {
        kind: "ServiceAccount".to_string(),
        name: sa_name.to_string(),
        namespace: Some(ns.to_owned()),
        ..Subject::default()
    };

    let metadata = ObjectMeta {
        name: Some(name.to_owned()),
        namespace: Some(ns.to_owned()),
        labels: Some(labels.clone()),
        ..ObjectMeta::default()
    };

    let rb = RoleBinding {
        metadata,
        role_ref,
        subjects: Some(vec![subject]),
    };

    let ps = PatchParams::apply("cntrlr").force();
    let _o = role_binding_api
        .patch(&name, &ps, &Patch::Apply(&rb))
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}

// Create role policy rulesets
async fn create_policy_rules() -> Vec<PolicyRule> {
    vec![
        // This policy allows create, get, list for pods & pods/exec
        PolicyRule {
            api_groups: Some(vec!["".to_owned()]),
            resources: Some(vec!["pods".to_owned(), "pods/exec".to_owned()]),
            verbs: vec!["create".to_string(), "get".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
    ]
}
