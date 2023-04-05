//! Functionality for downloading extensions and maintaining download counts
use crate::config::Config;
use crate::uploader::extension_location;
use actix_web::{get, web, HttpResponse, Responder};

/// Handles the `GET /extensions/:extension_name/:version/download` route.
/// This returns a URL to the location where the extension is stored.
#[get("/extensions/{extension_name}/{version}/download")]
pub async fn download(cfg: web::Data<Config>, path: web::Path<(String, String)>) -> impl Responder {
    let (name, version) = path.into_inner();
    // TODO(ianstanton) Increment download count for extension
    // TODO(ianstanton) Use latest version if none provided
    let url = extension_location(&cfg.bucket_name, &name, &version);
    HttpResponse::Ok().body(url)
}
