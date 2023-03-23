#[derive(Clone, Debug)]
pub enum Uploader {
    S3 {
        bucket: Box<s3::Bucket>,
        index_bucket: Option<Box<s3::Bucket>>,
        cdn: Option<String>,
    },

    /// Optional local configuration for development
    Local,
}

impl Uploader {
    /// Returns the URL of an uploaded extension's version archive.
    ///
    /// The function doesn't check for the existence of the file.
    pub fn extension_location(&self, extension_name: &str, version: &str) -> String {
        match *self {
            Uploader::S3 {
                ref bucket,
                ref cdn,
                ..
            } => {
                let host = match *cdn {
                    Some(ref s) => s.clone(),
                    None => bucket.host(),
                };
                let path = Uploader::extension_path(extension_name, version);
                format!("https://{host}/{path}")
            }
            Uploader::Local => format!("/{}", Uploader::extension_path(extension_name, version)),
        }
    }

    /// Returns the internal path of an uploaded extension's version archive.
    fn extension_path(name: &str, version: &str) -> String {
        format!("extensions/{name}/{name}-{version}.tar.gz")
    }
}
