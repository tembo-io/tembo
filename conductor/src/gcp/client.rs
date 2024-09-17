use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::buckets::{
    get_iam_policy::GetIamPolicyRequest, set_iam_policy::SetIamPolicyRequest, Policy,
};
use google_cloud_storage::http::Error as GcsError;

pub struct GcpStorageClient {
    client: Client,
    project_id: String,
    project_number: String,
}

fn generate_client_config(project_id: &str) -> ClientConfig {
    let project_id = Some(project_id.to_string());
    ClientConfig {
        project_id,
        ..Default::default()
    }
}

impl GcpStorageClient {
    /// Creates a new GcpStorageClient.
    ///
    /// # Errors
    ///
    /// Returns a `GcsError` if the client cannot be initialized.
    pub async fn new(project_id: &str, project_number: &str) -> Result<Self, GcsError> {
        let config = generate_client_config(project_id)
            .with_auth()
            .await
            .map_err(|e| GcsError::TokenSource(Box::new(e)))?;
        let client = Client::new(config);
        Ok(Self {
            client,
            project_id: project_id.to_string(),
            project_number: project_number.to_string(),
        })
    }

    /// Retrieves the project ID associated with the client.
    ///
    /// # Returns the project ID as a &str.
    pub fn get_project_id(&self) -> &str {
        &self.project_id
    }

    /// Retrieves the project number associated with the client.
    ///
    /// # Returns the project ID as a &str.
    pub fn get_project_number(&self) -> &str {
        &self.project_number
    }

    /// Retrieves the IAM policy for the specified bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket to retrieve the policy for.
    ///
    /// # Errors
    ///
    /// Returns a `GcsError` if the policy cannot be retrieved.
    pub async fn get_iam_policy(&self, bucket_name: &str) -> Result<Policy, GcsError> {
        let request = GetIamPolicyRequest {
            resource: bucket_name.to_string(),
            options_requested_policy_version: Some(3),
        };
        self.client.get_iam_policy(&request).await
    }

    /// Sets the IAM policy for the specified bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket to set the policy for.
    /// * `policy` - The policy to set.
    ///
    /// # Errors
    ///
    /// Returns a `GcsError` if the policy cannot be set.
    pub async fn set_iam_policy(
        &self,
        bucket_name: &str,
        policy: &Policy,
    ) -> Result<Policy, GcsError> {
        let request = SetIamPolicyRequest {
            resource: bucket_name.to_string(),
            policy: policy.clone(),
        };
        self.client.set_iam_policy(&request).await
    }
}
