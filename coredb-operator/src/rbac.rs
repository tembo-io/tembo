use crate::{apis::coredb_types::CoreDB, Context, Error};
use k8s_openapi::{
    api::{
        core::v1::ServiceAccount,
        rbac::v1::{PolicyRule, Role, RoleBinding, RoleRef, Subject},
    },
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    api::{Patch, PatchParams},
    Api, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc, vec};
//use tracing::debug;

// reconcile kubernetes rbac resources
pub async fn reconcile_rbac(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    // reconcile service account
    let sa = reconcile_service_account(cdb, ctx.clone()).await?;
    // reconcile role
    let role = reconcile_role(cdb, ctx.clone()).await?;
    // reconcile role binding
    reconcile_role_binding(cdb, ctx.clone(), sa, role).await?;

    Ok(())
}

// reconcile a kubernetes service account
async fn reconcile_service_account(cdb: &CoreDB, ctx: Arc<Context>) -> Result<ServiceAccount, Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-sa", cdb.name_any());
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

async fn reconcile_role(cdb: &CoreDB, ctx: Arc<Context>) -> Result<Role, Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-role", cdb.name_any());
    let role_api: Api<Role> = Api::namespaced(client.clone(), &ns);

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "coredb".to_string());
    labels.insert("coredb.io/name".to_owned(), cdb.name_any());

    let rules = create_policy_rules(cdb);

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
    sa: ServiceAccount,
    role: Role,
) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let name = format!("{}-role-binding", cdb.name_any());
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
async fn create_policy_rules(cdb: &CoreDB) -> Vec<PolicyRule> {
    vec![
        // This policy allows get, list, watch access to the coredb resource
        PolicyRule {
            api_groups: Some(vec!["coredb.io".to_owned()]),
            resource_names: Some(vec![cdb.name_any()]),
            resources: Some(vec!["coredb".to_owned()]),
            verbs: vec!["get".to_string(), "list".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
        // This policy allows get, patch, update, watch access to the coredb/status resource
        PolicyRule {
            api_groups: Some(vec!["coredb.io".to_owned()]),
            resource_names: Some(vec![cdb.name_any()]),
            resources: Some(vec!["coredb/status".to_owned()]),
            verbs: vec![
                "get".to_string(),
                "patch".to_string(),
                "update".to_string(),
                "watch".to_string(),
            ],
            ..PolicyRule::default()
        },
        // This policy allows get, watch access to a secret in the namespace
        PolicyRule {
            api_groups: Some(vec!["".to_owned()]),
            resource_names: Some(vec![format!("{}-connection", cdb.name_any())]),
            resources: Some(vec!["secrets".to_owned()]),
            verbs: vec!["get".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
        // This policy for now is specifically open for all configmaps in the namespace
        // We currently do not have any configmaps
        PolicyRule {
            api_groups: Some(vec!["".to_owned()]),
            resources: Some(vec!["configmaps".to_owned()]),
            verbs: vec!["get".to_string(), "watch".to_string()],
            ..PolicyRule::default()
        },
    ]
}
