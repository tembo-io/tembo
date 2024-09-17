use crate::gcp::{client::GcpStorageClient, iam_builder::IamBindingBuilder};
use google_cloud_storage::http::buckets::{Binding, Condition, Policy};
use google_cloud_storage::http::Error as GcsError;
use log::{info, warn};
use std::collections::HashMap;

const BUCKET_PATH_PREFIX: &str = "v2";
const GCP_STORAGE_ROLE: &str = "projects/{}/roles/TemboInstanceGCSRole";

/// Manages IAM policies for GCP storage buckets.
///
/// This struct provides methods to add and remove service account bindings
/// to/from GCP storage bucket IAM policies.
pub struct BucketIamManager {
    gcp_client: GcpStorageClient,
}

impl BucketIamManager {
    /// Creates a new `BucketIamManager` instance.
    ///
    /// # Arguments
    ///
    /// * `gcp_client` - An instance of `GcpStorageClient` used for interacting with GCP storage.
    pub fn new(gcp_client: GcpStorageClient) -> Self {
        Self { gcp_client }
    }

    /// Adds a service account binding to the specified buckets' IAM policies.
    ///
    /// # Arguments
    ///
    /// * `buckets` - A vector of bucket names to add the binding to.
    /// * `namespace` - The namespace of the service account.
    /// * `service_account` - The name of the service account to add.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a HashMap of bucket names to their updated `Policy` if successful,
    /// or a `GcsError` if any operation fails.
    pub async fn add_service_account_binding(
        &self,
        buckets: Vec<&str>,
        namespace: &str,
        service_account: &str,
    ) -> Result<HashMap<String, Policy>, GcsError> {
        let member = self.create_member_string(namespace, service_account);
        let instance_name = namespace;
        let mut results = HashMap::new();

        for bucket_name in buckets {
            let condition = self.create_bucket_condition(bucket_name, instance_name);
            let mut policy = self.gcp_client.get_iam_policy(bucket_name).await?;

            if self.binding_exists(&policy, &member, &condition) {
                info!("Binding already exists for {} in bucket {} with the correct role and condition. No changes needed.", member, bucket_name);
                results.insert(bucket_name.to_string(), policy);
                continue;
            }

            self.update_or_create_binding(&mut policy, &member, condition);

            // Set policy version to 3 to ensure the condition can be applied.
            // for more information see: https://cloud.google.com/iam/docs/policies#versions
            policy.version = 3;

            let updated_policy = self.gcp_client.set_iam_policy(bucket_name, &policy).await?;
            results.insert(bucket_name.to_string(), updated_policy);
            info!(
                "Successfully added binding for {} to bucket {}",
                member, bucket_name
            );
        }

        Ok(results)
    }

    /// Gets the GCP_STORAGE_ROLE with the project ID filled in.
    ///
    /// # Returns
    ///
    /// Returns the GCP_STORAGE_ROLE as a String with the project ID filled in.
    fn get_storage_role(&self) -> String {
        GCP_STORAGE_ROLE.replace("{}", self.gcp_client.get_project_id())
    }

    /// Checks if a binding with the specified member and condition already exists in the policy.
    ///
    /// # Arguments
    ///
    /// * `policy` - The current IAM policy.
    /// * `member` - The member string to check for.
    /// * `condition` - The condition to check for.
    ///
    /// # Returns
    ///
    /// Returns `true` if the binding exists, `false` otherwise.
    fn binding_exists(&self, policy: &Policy, member: &str, condition: &Condition) -> bool {
        let role = self.get_storage_role();
        policy.bindings.iter().any(|b| {
            b.role == role
                && b.condition.as_ref() == Some(condition)
                && b.members.contains(&member.to_string())
        })
    }

    /// Updates an existing binding or creates a new one if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `policy` - The IAM policy to update.
    /// * `member` - The member string to add to the binding.
    /// * `condition` - The condition for the binding.
    fn update_or_create_binding(&self, policy: &mut Policy, member: &str, condition: Condition) {
        let role = self.get_storage_role();
        if let Some(binding) = self.find_matching_binding(policy, &condition) {
            if !binding.members.contains(&member.to_string()) {
                binding.members.push(member.to_string());
                info!("Added {} to existing binding.", member);
            } else {
                warn!(
                    "Member {} already exists in the binding. No changes made.",
                    member
                );
            }
        } else {
            let new_binding = self.create_new_binding(member.to_string(), condition);
            policy.bindings.push(new_binding);
            info!(
                "Created new binding for {} with role {} and condition.",
                member, role
            );
        }
    }

    /// Finds a matching binding in the policy based on the role and condition.
    ///
    /// # Arguments
    ///
    /// * `policy` - The IAM policy to search.
    /// * `condition` - The condition to match.
    ///
    /// # Returns
    ///
    /// Returns an `Option` containing a mutable reference to the matching `Binding` if found.
    fn find_matching_binding<'a>(
        &self,
        policy: &'a mut Policy,
        condition: &Condition,
    ) -> Option<&'a mut Binding> {
        let role = self.get_storage_role();
        policy
            .bindings
            .iter_mut()
            .find(|b| b.role == role && b.condition.as_ref() == Some(condition))
    }

    /// Creates a new binding with the specified member and condition.
    ///
    /// # Arguments
    ///
    /// * `member` - The member string to add to the new binding.
    /// * `condition` - The condition for the new binding.
    ///
    /// # Returns
    ///
    /// Returns a new `Binding` instance.
    fn create_new_binding(&self, member: String, condition: Condition) -> Binding {
        let role = self.get_storage_role();
        IamBindingBuilder::new()
            .role(role)
            .add_member(member)
            .condition(condition)
            .build()
            .expect("Failed to build binding")
    }

    /// Removes a service account binding from the specified buckets' IAM policies.
    ///
    /// # Arguments
    ///
    /// * `buckets` - A vector of bucket names to remove the binding from.
    /// * `namespace` - The namespace of the service account.
    /// * `service_account` - The name of the service account to remove.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a HashMap of bucket names to their updated `Policy` if successful,
    /// or a `GcsError` if any operation fails.
    pub async fn remove_service_account_binding(
        &self,
        buckets: Vec<&str>,
        namespace: &str,
        service_account: &str,
    ) -> Result<HashMap<String, Policy>, GcsError> {
        let member = self.create_member_string(namespace, service_account);
        let role = self.get_storage_role();
        let mut results = HashMap::new();

        for bucket_name in buckets {
            let mut policy = self.gcp_client.get_iam_policy(bucket_name).await?;

            let initial_binding_count = policy.bindings.len();
            let mut removed = false;

            policy.bindings = policy
                .bindings
                .into_iter()
                .filter_map(|mut binding| {
                    if binding.role == role {
                        binding.members.retain(|m| m != &member);
                        if binding.members.is_empty() {
                            removed = true;
                            None
                        } else {
                            Some(binding)
                        }
                    } else {
                        Some(binding)
                    }
                })
                .collect();

            if removed || policy.bindings.len() < initial_binding_count {
                info!(
                    "Removed binding for {} from policy in bucket {}.",
                    member, bucket_name
                );
            } else {
                warn!(
                    "No binding found for {} in bucket {}. Policy unchanged.",
                    member, bucket_name
                );
            }

            let updated_policy = self.gcp_client.set_iam_policy(bucket_name, &policy).await?;
            results.insert(bucket_name.to_string(), updated_policy);
        }

        Ok(results)
    }

    /// Creates a member string for a service account.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace of the service account.
    /// * `service_account` - The name of the service account.
    ///
    /// # Returns
    ///
    /// Returns a formatted string representing the member.
    fn create_member_string(&self, namespace: &str, service_account: &str) -> String {
        let project_id = self.gcp_client.get_project_id();
        let project_number = self.gcp_client.get_project_number();
        format!(
            "principal://iam.googleapis.com/projects/{}/locations/global/workloadIdentityPools/{}.svc.id.goog/subject/ns/{}/sa/{}",
            project_number, project_id, namespace, service_account
        )
    }

    /// Creates a bucket condition for IAM policies.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the GCP storage bucket.
    ///
    /// # Returns
    ///
    /// Returns a `Condition` instance for the specified bucket.
    fn create_bucket_condition(&self, bucket_name: &str, instance_name: &str) -> Condition {
        Condition {
            title: "allow-bucket-and-path".to_string(),
            description: "Conductor managed storage bucket IAM policy condition".to_string(),
            expression: format!(
                r#"(resource.type == "storage.googleapis.com/Bucket") || (resource.type == "storage.googleapis.com/Object" && resource.name.startsWith("projects/_/buckets/{}/objects/{}/{}"))"#,
                bucket_name, BUCKET_PATH_PREFIX, instance_name
            ),
        }
    }
}
