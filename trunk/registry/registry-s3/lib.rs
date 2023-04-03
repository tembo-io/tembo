#![warn(clippy::all, rust_2018_idioms)]

use chrono::prelude::Utc;
use hmac::{Hmac, Mac};
use reqwest::{{Body, Client, Response},
    header,
};
use sha1::Sha1;
use std::time::Duration;

pub use reqwest::Error;

#[derive(Clone, Debug)]
pub struct Bucket {
    name: String,
    region: Option<String>,
    access_key: String,
    secret_key: String,
    proto: String,
}

impl Bucket {
    pub fn new(
        name: &String,
        region: &Option<String>,
        access_key: &String,
        secret_key: &String,
        proto: &str,
    ) -> Bucket {
        Bucket {
            name: name.to_string(),
            region: region.clone(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            proto: proto.to_string(),
        }
    }

    pub async fn put<R: Into<Body>>(
        &self,
        client: &Client,
        path: &str,
        content: R,
        content_type: &str,
        extra_headers: header::HeaderMap,
    ) -> Result<Response, Error> {
        let path = path.strip_prefix('/').unwrap_or(path);
        let date = Utc::now().to_rfc2822();
        let auth = self.auth("PUT", &date, path, "", content_type);
        let url = self.url(path);

        client
            .put(url)
            .header(header::AUTHORIZATION, auth)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::DATE, date)
            .header(header::USER_AGENT, "pgtrunk.io (https://pgtrunk.io)")
            .headers(extra_headers)
            .body(content.into())
            .timeout(Duration::from_secs(60))
            .send().await?
            .error_for_status()
            .map_err(Into::into)
    }

    pub async fn delete(&self, client: &Client, path: &str) -> Result<Response, Error> {
        let path = path.strip_prefix('/').unwrap_or(path);
        let date = Utc::now().to_rfc2822();
        let auth = self.auth("DELETE", &date, path, "", "");
        let url = self.url(path);

        client
            .delete(url)
            .header(header::DATE, date)
            .header(header::AUTHORIZATION, auth)
            .send().await?
            .error_for_status()
            .map_err(Into::into)
    }

    pub fn host(&self) -> String {
        format!(
            "{}.s3{}.amazonaws.com",
            self.name,
            match self.region {
                Some(ref r) if !r.is_empty() => format!("-{r}"),
                Some(_) => String::new(),
                None => String::new(),
            }
        )
    }

    fn auth(&self, verb: &str, date: &str, path: &str, md5: &str, content_type: &str) -> String {
        let string = format!(
            "{verb}\n{md5}\n{ty}\n{date}\n{headers}/{name}/{path}",
            ty = content_type,
            headers = "",
            name = self.name,
        );
        let signature = {
            let key = self.secret_key.as_bytes();
            let mut h = Hmac::<Sha1>::new_from_slice(key).expect("HMAC can take key of any size");
            h.update(string.as_bytes());
            let res = h.finalize().into_bytes();
            base64::encode(res)
        };
        format!("AWS {}:{}", self.access_key, signature)
    }

    fn url(&self, path: &str) -> String {
        format!("{}://{}/{}", self.proto, self.host(), path)
    }
}
