// Add this enum at the top of your file or in a separate module
pub struct CloudProviderBuilder {
    gcp: bool,
    aws: bool,
    azure: bool,
}

impl CloudProviderBuilder {
    fn new() -> Self {
        CloudProviderBuilder {
            gcp: false,
            aws: false,
            azure: false,
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

    pub fn azure(mut self, value: bool) -> Self {
        self.azure = value;
        self
    }

    pub fn build(self) -> CloudProvider {
        if self.gcp {
            CloudProvider::GCP
        } else if self.aws {
            CloudProvider::AWS
        } else if self.azure {
            CloudProvider::Azure
        } else {
            CloudProvider::Unknown
        }
    }
}

pub enum CloudProvider {
    AWS,
    Azure,
    GCP,
    Unknown,
}

impl CloudProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloudProvider::AWS => "aws",
            CloudProvider::Azure => "azure",
            CloudProvider::GCP => "gcp",
            CloudProvider::Unknown => "unknown",
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            CloudProvider::AWS => "s3://",
            CloudProvider::Azure => "https://",
            CloudProvider::GCP => "gs://",
            CloudProvider::Unknown => "",
        }
    }

    pub fn builder() -> CloudProviderBuilder {
        CloudProviderBuilder::new()
    }
}
