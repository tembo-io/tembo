use crate::{apis::coredb_types::CoreDB, Context, Error};
use k8s_openapi::api::{
    core::v1::ServiceAccount,
    rbac::v1::{Role, RoleBinding},
};
use kube::{api::ListParams, Api, ResourceExt};
use std::sync::Arc;
use tracing::{debug, error};

// pub struct Rbac {
//     pub service_account: ServiceAccount,
//     pub role: Role,
//     pub rolebinding: RoleBinding,
// }

// Delete the postgres-exporter RBAC objects from the cluster
pub async fn cleanup_postgres_exporter_rbac(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    delete_postgres_exporter_service_account(cdb, ctx.clone()).await?;
    delete_postgres_exporter_role(cdb, ctx.clone()).await?;
    delete_postgres_exporter_role_binding(cdb, ctx.clone()).await?;
    Ok(())
}

// Delete the postgres-exporter ServiceAccount from the cluster
async fn delete_postgres_exporter_service_account(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let service_account_api: Api<ServiceAccount> = Api::namespaced(client, &ns);

    // Define the label selector based on your service account labels
    let label_selector = "app=coredb,coredb.io/name=".to_owned() + &cdb.name_any();
    let lp = ListParams::default().labels(&label_selector);

    // List service accounts with specified labels
    let service_accounts = service_account_api.list(&lp).await?;

    // Delete the service account
    for service_account in service_accounts {
        if let Some(service_account_name) = service_account.metadata.name {
            match service_account_api
                .delete(&service_account_name, &Default::default())
                .await
            {
                Ok(_) => {
                    debug!(
                        "Deleted ServiceAccount: {}, for instance {}",
                        service_account_name,
                        &cdb.name_any()
                    );
                }
                Err(e) => {
                    error!(
                        "Error deleting ServiceAccount: {}, for instance {}",
                        e,
                        &cdb.name_any()
                    );
                    return Err(Error::KubeError(e));
                }
            }
        }
    }

    Ok(())
}

// Delete the postgres-exporter Role from the cluster
async fn delete_postgres_exporter_role(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let role_api: Api<Role> = Api::namespaced(client, &ns);

    // Define the label selector based on your role labels
    let label_selector = "app=coredb,coredb.io/name=".to_owned() + &cdb.name_any();
    let lp = ListParams::default().labels(&label_selector);

    // List roles with specified labels
    let roles = role_api.list(&lp).await?;

    // Delete the role
    for role in roles {
        if let Some(role_name) = role.metadata.name {
            match role_api.delete(&role_name, &Default::default()).await {
                Ok(_) => {
                    debug!(
                        "Deleted Role: {} for instance {}",
                        role_name,
                        &cdb.name_any()
                    );
                }
                Err(e) => {
                    error!(
                        "Error deleting Role: {}, for instance {}",
                        e,
                        &cdb.name_any()
                    );
                    return Err(Error::KubeError(e));
                }
            }
        }
    }

    Ok(())
}

// Delete the postgres-exporter RoleBinding from the cluster
async fn delete_postgres_exporter_role_binding(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let role_binding_api: Api<RoleBinding> = Api::namespaced(client, &ns);

    // Define the label selector based on your role binding labels
    let label_selector = "app=coredb,coredb.io/name=".to_owned() + &cdb.name_any();
    let lp = ListParams::default().labels(&label_selector);

    // List role bindings with specified labels
    let role_bindings = role_binding_api.list(&lp).await?;

    // Delete the role binding
    for role_binding in role_bindings {
        if let Some(role_binding_name) = role_binding.metadata.name {
            match role_binding_api
                .delete(&role_binding_name, &Default::default())
                .await
            {
                Ok(_) => {
                    debug!(
                        "Deleted RoleBinding: {}, for instance {}",
                        role_binding_name,
                        &cdb.name_any()
                    );
                }
                Err(e) => {
                    error!(
                        "Error deleting RoleBinding: {}, for instance {}",
                        e,
                        &cdb.name_any()
                    );
                    return Err(Error::KubeError(e));
                }
            }
        }
    }

    Ok(())
}

// // reconcile kubernetes rbac resources
// pub async fn reconcile_rbac(
//     cdb: &CoreDB,
//     ctx: Arc<Context>,
//     suffix: Option<&str>,
//     policy_rules: Vec<PolicyRule>,
// ) -> Result<Rbac, Error> {
//     // reconcile service account
//     let service_account = reconcile_service_account(cdb, ctx.clone(), suffix).await?;
//     let sa = service_account.clone();
//     // reconcile role
//     let role = reconcile_role(cdb, ctx.clone(), suffix, policy_rules).await?;
//     let rle = role.clone();
//     // reconcile role binding
//     let role_binding = reconcile_role_binding(cdb, ctx.clone(), service_account, rle.clone(), suffix).await?;
//
//     Ok(Rbac {
//         service_account: sa,
//         role: rle,
//         rolebinding: role_binding,
//     })
// }
//
// // reconcile a kubernetes service account
// async fn reconcile_service_account(
//     cdb: &CoreDB,
//     ctx: Arc<Context>,
//     suffix: Option<&str>,
// ) -> Result<ServiceAccount, Error> {
//     let suffix = suffix.map_or("sa".to_owned(), |s| {
//         if s.is_empty() {
//             "sa".to_owned()
//         } else {
//             s.to_owned()
//         }
//     });
//     let client = ctx.client.clone();
//     let ns = cdb.namespace().unwrap();
//     let name = format!("{}-{}", cdb.name_any(), suffix);
//     let sa_api: Api<ServiceAccount> = Api::namespaced(client.clone(), &ns);
//
//     let mut labels: BTreeMap<String, String> = BTreeMap::new();
//     labels.insert("app".to_owned(), "coredb".to_string());
//     labels.insert("coredb.io/name".to_owned(), cdb.name_any());
//
//     let mut sa_metadata = ObjectMeta {
//         name: Some(name.to_owned()),
//         namespace: Some(ns.to_owned()),
//         labels: Some(labels.clone()),
//         ..ObjectMeta::default()
//     };
//
//     if let Some(ref template_metadata) = cdb.spec.serviceAccountTemplate.metadata {
//         if let Some(ref annotations) = template_metadata.annotations {
//             sa_metadata.annotations = Some(annotations.clone());
//         }
//     }
//
//     let sa = ServiceAccount {
//         metadata: sa_metadata,
//         ..ServiceAccount::default()
//     };
//
//     let ps = PatchParams::apply("cntrlr").force();
//     let _o = sa_api
//         .patch(&name, &ps, &Patch::Apply(&sa))
//         .await
//         .map_err(Error::KubeError)?;
//
//     Ok(sa)
// }
//
// async fn reconcile_role(
//     cdb: &CoreDB,
//     ctx: Arc<Context>,
//     suffix: Option<&str>,
//     policy_rules: Vec<PolicyRule>,
// ) -> Result<Role, Error> {
//     let suffix = suffix.map_or("role".to_owned(), |s| {
//         if s.is_empty() {
//             "role".to_owned()
//         } else {
//             s.to_owned()
//         }
//     });
//     let client = ctx.client.clone();
//     let ns = cdb.namespace().unwrap();
//     let name = format!("{}-{}", cdb.name_any(), suffix);
//     let role_api: Api<Role> = Api::namespaced(client.clone(), &ns);
//
//     let mut labels: BTreeMap<String, String> = BTreeMap::new();
//     labels.insert("app".to_owned(), "coredb".to_string());
//     labels.insert("coredb.io/name".to_owned(), cdb.name_any());
//
//     let role = Role {
//         metadata: ObjectMeta {
//             name: Some(name.to_owned()),
//             namespace: Some(ns.to_owned()),
//             labels: Some(labels.clone()),
//             ..ObjectMeta::default()
//         },
//         rules: Some(policy_rules.to_vec()),
//     };
//
//     let ps = PatchParams::apply("cntrlr").force();
//     let _o = role_api
//         .patch(&name, &ps, &Patch::Apply(&role))
//         .await
//         .map_err(Error::KubeError)?;
//
//     Ok(role)
// }
//
// async fn reconcile_role_binding(
//     cdb: &CoreDB,
//     ctx: Arc<Context>,
//     sa: ServiceAccount,
//     role: Role,
//     suffix: Option<&str>,
// ) -> Result<RoleBinding, Error> {
//     let suffix = suffix.map_or("role-binding".to_owned(), |s| {
//         if s.is_empty() {
//             "role-binding".to_owned()
//         } else {
//             s.to_owned()
//         }
//     });
//     let client = ctx.client.clone();
//     let ns = cdb.namespace().unwrap();
//     let name = format!("{}-{}", cdb.name_any(), suffix);
//     let role_binding_api: Api<RoleBinding> = Api::namespaced(client.clone(), &ns);
//     let sa_name = sa.name_any();
//     let role_name = role.name_any();
//
//     let mut labels: BTreeMap<String, String> = BTreeMap::new();
//     labels.insert("app".to_owned(), "coredb".to_string());
//     labels.insert("coredb.io/name".to_owned(), cdb.name_any());
//
//     let role_ref = RoleRef {
//         api_group: "rbac.authorization.k8s.io".to_string(),
//         kind: "Role".to_string(),
//         name: role_name.to_string(),
//     };
//
//     let subject = Subject {
//         kind: "ServiceAccount".to_string(),
//         name: sa_name.to_string(),
//         namespace: Some(ns.to_owned()),
//         ..Subject::default()
//     };
//
//     let metadata = ObjectMeta {
//         name: Some(name.to_owned()),
//         namespace: Some(ns.to_owned()),
//         labels: Some(labels.clone()),
//         ..ObjectMeta::default()
//     };
//
//     let rb = RoleBinding {
//         metadata,
//         role_ref,
//         subjects: Some(vec![subject]),
//     };
//
//     let ps = PatchParams::apply("cntrlr").force();
//     let _o = role_binding_api
//         .patch(&name, &ps, &Patch::Apply(&rb))
//         .await
//         .map_err(Error::KubeError)?;
//
//     Ok(rb)
// }
