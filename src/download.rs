//! Functionality for downloading extensions and maintaining download counts
use crate::config::Config;
use crate::uploader;
use actix_web::{get, web, HttpResponse, Responder};

/// Handles the `GET /extensions/:extension_name/:version/download` route.
/// This returns a URL to the location where the extension is stored.
#[get("/extensions/{extension_name}/{version}/download")]
pub async fn download(cfg: web::Data<Config>, path: web::Path<(String, String)>) -> impl Responder {
    let (name, version) = path.into_inner();
    let bucket = uploader::Uploader::S3 {
        bucket: Box::new(s3::Bucket::new(
            cfg.bucket_name.to_string(),
            Option::from(cfg.region.to_string()),
            cfg.aws_access_key.to_string(),
            cfg.aws_secret_key.to_string(),
            "https",
        )),
        index_bucket: None,
        cdn: None,
    };
    // TODO(ianstanton) Increment download count for extension
    // TODO(ianstanton) Use latest version if none provided
    let url = bucket.extension_location(&name, &version);
    HttpResponse::Ok().body(url)
}
