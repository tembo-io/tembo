use actix_web::web;

use crate::routes;

pub fn webserver_routes(configuration: &mut web::ServiceConfig) {
    configuration
        .service(routes::health::ready)
        .service(routes::health::lively)
        .default_service(web::to(routes::forward::forward_request));
}
