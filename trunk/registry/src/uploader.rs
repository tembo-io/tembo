use crate::errors::ExtensionRegistryError;
use crate::views::extension_publish::ExtensionUpload;
use reqwest::header;
use reqwest::{Body, Client};
use std::fs::File;
use std::path::PathBuf;
use std::{env, fs};

const CACHE_CONTROL_IMMUTABLE: &str = "public,max-age=31536000,immutable";

#[derive(Clone, Debug)]
pub enum Uploader {
    S3 {
        bucket: Box<s3::Bucket>,
        cdn: Option<String>,
    },

    /// Optional local configuration for development
    Local,
}

pub enum UploadBucket {
    Default,
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

    /// Returns the absolute path to the locally uploaded file.
    fn local_uploads_path(path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        env::current_dir().unwrap().join("local_uploads").join(path)
    }

    pub async fn upload<R: Into<Body>>(
        &self,
        client: &Client,
        path: &str,
        content: R,
        content_type: &str,
        extra_headers: header::HeaderMap,
    ) -> Result<Option<String>, ExtensionRegistryError> {
        match *self {
            Uploader::S3 { ref bucket, .. } => {
                let bucket = Some(bucket);

                if let Some(bucket) = bucket {
                    bucket
                        .put(client, path, content, content_type, extra_headers)
                        .await?;
                }

                Ok(Some(String::from(path)))
            }
            Uploader::Local => {
                let filename = Self::local_uploads_path(path);
                let dir = filename.parent().unwrap();
                fs::create_dir_all(dir)?;
                let mut file = File::create(&filename)?;
                let body = content.into();
                let mut buffer = body.as_bytes().unwrap();
                std::io::copy(&mut buffer, &mut file)?;
                println!("Uploading to {:?}", filename);
                Ok(filename.to_str().map(String::from))
            }
        }
    }

    /// Uploads an extension file.
    pub async fn upload_extension<R: Into<Body>>(
        &self,
        http_client: &Client,
        body: R,
        extension: &ExtensionUpload,
        vers: &semver::Version,
    ) -> Result<String, ExtensionRegistryError> {
        let path = Uploader::extension_path(&extension.name, &vers.to_string());
        let mut extra_headers = header::HeaderMap::new();
        extra_headers.insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static(CACHE_CONTROL_IMMUTABLE),
        );
        println!("Uploading");
        self.upload(http_client, &path, body, "application/gzip", extra_headers)
            .await?;
        Ok("test".to_owned())
    }
}
