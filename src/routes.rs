use crate::config::Config;
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
pub async fn get_extensions(cfg: web::Data<Config>) -> impl Responder {
    let mut extensions: Vec<&Object> = Vec::new();
    let mut credentials = Credentials::default().unwrap();
    if (!cfg.aws_access_key.is_empty()) && (!cfg.aws_secret_key.is_empty()) {
        credentials = Credentials::new(
            Some(&cfg.aws_access_key),
            Some(&cfg.aws_secret_key),
            None,
            None,
            None,
        )
        .unwrap();
    }
    let list: Result<Vec<ListBucketResult>, S3Error> =
        s3_list(&cfg.bucket_name, &cfg.region, credentials).await;
    let ulist = list.unwrap();
    for ext in ulist[0].contents.iter() {
        extensions.push(ext);
    }
    HttpResponse::Ok().body(format!("Extensions... {:?}", extensions))
}

pub async fn s3_list(
    bucket_name: &str,
    region: &str,
    credentials: Credentials,
) -> Result<Vec<ListBucketResult>, S3Error> {
    let region = region.parse()?;
    let bucket = Bucket::new(bucket_name, region, credentials)?;
    let list = bucket.list("".to_string(), Some("".to_string())).await?;
    Ok::<Vec<ListBucketResult>, S3Error>(list)
}
