use crate::{apis::coredb_types::CoreDB, rbac::reconcile_rbac, Context, Error};
use k8s_openapi::{
    api::{
        batch::v1::{CronJob, CronJobSpec, JobSpec, JobTemplateSpec},
        core::v1::{Container, PodSpec, PodTemplateSpec},
        rbac::v1::PolicyRule,
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

    let rbac = reconcile_rbac(cdb, ctx.clone(), Some("backup"), create_policy_rules().await).await?;

    // reconcile cronjob
    let cronjob_metadata = ObjectMeta {
        name: Some(name.to_owned()),
        namespace: Some(ns.to_owned()),
        labels: Some(labels.clone()),
        owner_references: Some(vec![oref]),
        ..ObjectMeta::default()
    };

    let sa_name = rbac.service_account.metadata.name;

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
