use crate::config::S3Config;
use actix_web::{get, web, HttpResponse, Responder};
use anyhow::Result;
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::error::S3Error;
use s3::serde_types::{ListBucketResult, Object};

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("API is up and running!")
}

// get all extensions in s3 bucket
#[get("/get-extensions")]
pub async fn get_extensions(cfg: web::Data<S3Config>) -> impl Responder {
    let mut extensions: Vec<&Object> = Vec::new();
    let list: Result<Vec<ListBucketResult>, S3Error> = s3_list(&cfg.bucket_name, &cfg.region).await;
    let ulist = list.unwrap();
    for ext in ulist[0].contents.iter() {
        extensions.push(ext);
    }
    HttpResponse::Ok().body(format!("Extensions... {:?}", extensions))
}

pub async fn s3_list(bucket_name: &str, region: &str) -> Result<Vec<ListBucketResult>, S3Error> {
    let region = region.parse()?;
    // TODO(ianstanton) Allow for reading creds from env var
    let credentials = Credentials::default()?;
    let bucket = Bucket::new(bucket_name, region, credentials)?;
    let list = bucket.list("".to_string(), Some("".to_string())).await?;
    Ok::<Vec<ListBucketResult>, S3Error>(list)
}
