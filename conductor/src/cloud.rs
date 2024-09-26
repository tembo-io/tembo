// Add this enum at the top of your file or in a separate module
use crate::errors::ConductorError;

pub struct CloudProviderBuilder {
    gcp: bool,
    aws: bool,
}

impl CloudProviderBuilder {
    fn new() -> Self {
        CloudProviderBuilder {
            gcp: false,
            aws: false,
        }
    }

    pub fn gcp(mut self, value: bool) -> Self {
        self.gcp = value;
        self
    }

    pub fn aws(mut self, value: bool) -> Self {
        self.aws = value;
        self
    }

    pub fn build(self) -> Result<CloudProvider, ConductorError> {
        if self.gcp {
            Ok(CloudProvider::GCP)
        } else if self.aws {
            Ok(CloudProvider::AWS)
        } else {
            Err(ConductorError::DataplaneError(format!(
                "Unsupported cloud provider got : gcp: {}, aws: {}",
                self.gcp, self.aws
            )))
        }
    }
}

pub enum CloudProvider {
    AWS,
    GCP,
}

impl CloudProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloudProvider::AWS => "aws",
            CloudProvider::GCP => "gcp",
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            CloudProvider::AWS => "s3://",
            CloudProvider::GCP => "gs://",
        }
    }

    pub fn builder() -> CloudProviderBuilder {
        CloudProviderBuilder::new()
    }
}
