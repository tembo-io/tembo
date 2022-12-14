#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use actix_web::{App, HttpServer};
use dotenv::dotenv;
use listenfd::ListenFd;
use std::env;

mod db;
mod items;
mod error_handler;
mod schema;


#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    db::init();

    let mut listenfd = ListenFd::from_env();
    let mut server = HttpServer::new(|| App::new().configure(items::init_routes));

    server = match listenfd.take_tcp_listener(0)? {
        Some(listener) => server.listen(listener)?,
        None => {
            let host = env::var("WEBSERVER_HOST").expect("Set WEBSERVER_HOST in .env");
            let port = env::var("WEBSERVER_PORT").expect("Set WEBSERVER_PORT in .env");
            server.bind(format!("{}:{}", host, port))?
        }
    };

    server.run().await
}