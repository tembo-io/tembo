// Add this enum at the top of your file or in a separate module
pub struct CloudProviderBuilder {
    aws: bool,
}

impl CloudProviderBuilder {
    fn new() -> Self {
        CloudProviderBuilder { aws: false }
    }

    pub fn aws(mut self, value: bool) -> Self {
        self.aws = value;
        self
    }

    pub fn build(self) -> CloudProvider {
        if self.aws {
            CloudProvider::AWS
        } else {
            CloudProvider::Unknown
        }
    }
}

#[derive(PartialEq)]
pub enum CloudProvider {
    AWS,
    Unknown,
}

impl CloudProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloudProvider::AWS => "aws",
            CloudProvider::Unknown => "unknown",
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            CloudProvider::AWS => "s3://",
            CloudProvider::Unknown => "",
        }
    }

    pub fn builder() -> CloudProviderBuilder {
        CloudProviderBuilder::new()
    }
}
