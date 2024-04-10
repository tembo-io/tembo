use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

type SecretNameFormatter = Box<dyn Fn(&str) -> String + Send + Sync>;

#[derive(Serialize, ToSchema)]
pub struct AvailableSecret {
    /// The name of an available secret
    pub name: String,
    /// For this secret, available keys
    pub possible_keys: Vec<String>,
    // All secrets need a string formatting function
    #[serde(skip)]
    pub formatter: SecretNameFormatter,
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordString {
    /// The New Password
    pub password: String,
}

impl AvailableSecret {
    pub fn kube_secret_name(&self, instance_name: &str) -> String {
        (self.formatter)(instance_name)
    }
}
