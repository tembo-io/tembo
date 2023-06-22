use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    error::{Error as KubeError, ErrorResponse},
    Api, Client,
};

#[get("/health/liveness")]
pub async fn liveness(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("I'm alive!")
}

#[get("/health/readiness")]
pub async fn readiness(_: HttpRequest, client: web::Data<Client>) -> impl Responder {
    let pods: Api<Pod> = Api::all(client.as_ref().clone());
    let result = pods.list(&Default::default()).await;

    match result {
        Ok(_) => HttpResponse::Ok().json("I'm ready!"),
        Err(KubeError::Api(ErrorResponse { reason, .. })) if reason == "Unauthorized" => {
            HttpResponse::Unauthorized().json("I'm not ready!")
        }
        Err(_) => HttpResponse::InternalServerError().json("I'm not ready!"),
    }
}
