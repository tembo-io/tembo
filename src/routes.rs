use actix_web::{get, HttpResponse, Responder};

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("API is up and running!")
}

// get all extensions in s3 bucket
#[get("/get-extensions")]
pub async fn get_extensions() -> impl Responder {
    let mut extensions: Vec<String> = Vec::new();
    // pass in s3 config
    // return all extensions in s3 bucket
    extensions.push("ext1".to_owned());
    extensions.push("ext2".to_owned());
    extensions.push("ext3".to_owned());
    HttpResponse::Ok().body(format!("Extensions... {:?}", extensions))
}
