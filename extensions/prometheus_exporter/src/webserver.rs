// https://github.com/prometheus/client_rust/blob/master/examples/actix-web.rs
use std::sync::{Arc, Mutex};
use std::{thread, time};

use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result};

use tokio::{task}; // 1.3.0

use pgx::bgworkers::*;
use pgx::prelude::*;

use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

const UPTIME_QUERY: &str = "SELECT FLOOR(EXTRACT(EPOCH FROM now() - pg_postmaster_start_time))
FROM pg_postmaster_start_time();";

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum Method {
    Get,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct MethodLabels {
    pub method: Method,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct UptimeLabels {
    pub label: String,
}


pub struct Metrics {
    requests: Family<MethodLabels, Counter>,
    uptime: Family<(), Gauge>,
}

impl Metrics {
    pub fn inc_requests(&self, method: Method) {
        self.requests.get_or_create(&MethodLabels { method }).inc();
    }

    pub fn pg_uptime(&self) {
        let res = query();
        println!("res: {:?}", res);
        match res {
            Some(t) => self.uptime.get_or_create(&()).set(t),
            None => panic!("Could not get uptime from Postgres"),
        };
    }
}

pub struct AppState {
    pub registry: Registry,
}

pub async fn metrics_handler(state: web::Data<Mutex<AppState>>) -> Result<HttpResponse> {
    println!("metrics_handler called");
    let state = state.lock().unwrap();
    let mut body = String::new();
    encode(&mut body, &state.registry).unwrap();
    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body))
}


pub async fn test() -> impl Responder {
    "test".to_string()
}

pub async fn update_metrics(metrics_clone: Arc<Mutex<Metrics>>) -> impl Responder {
    println!("updating metrics");
    
    let metrics = metrics_clone.lock().unwrap();
    loop {
        // TODO: this needs to be interruptable
        metrics.pg_uptime();
        metrics.inc_requests(Method::Get);
        thread::sleep(time::Duration::from_millis(2500));
    }
    "done".to_string()
}

#[pg_extern(immutable, parallel_safe)]
fn query() -> Option<i64> {
    let uptime = Arc::new(Mutex::new(i64::default()));
    let clone = Arc::clone(&uptime);
    println!("query called");
    // BackgroundWorker::transaction(move || {
    //     let ut: Option<i64> = Spi::get_one(UPTIME_QUERY);
    //     match ut {
    //         Some(t) => println!("t: {:?}", t),
    //         None => println!("no value"),
    //     }
    // });
    BackgroundWorker::transaction(move || {
        Spi::execute(|client| {
            println!("query called (inside spi");
            let tuple_table = client.select(UPTIME_QUERY, None, None);
            println!("tuple_table: {:?}", tuple_table);
            for tup in tuple_table {
                let uptime = tup.by_name("floor").unwrap().value::<i64>().unwrap();
                let mut obj_clone = clone.lock().unwrap();
                *obj_clone = uptime;
                break
            }
        });
    });
    let x = Some(*uptime.lock().unwrap());
    println!("uptime: {:?}", x);
    x
}

#[actix_web::main]
pub async fn serve() -> std::io::Result<()> {
    let metrics = Metrics {
        requests: Family::default(),
        uptime: Family::default(),
    };
    
    let mut state = AppState {
        registry: Registry::default(),
    };
    
    state
    .registry
    .register("requests", "Count of requests", metrics.requests.clone());
    state.registry
    .register("pg_uptime", "Postgres server uptime", metrics.uptime.clone());
    let state = web::Data::new(Mutex::new(state));

    let mutex_metrics = Arc::new(Mutex::new(metrics));
    let metrics_clone = Arc::clone(&mutex_metrics);

    task::spawn(async { update_metrics(metrics_clone).await; });

    HttpServer::new(move || {
        App::new()
        .app_data(state.clone())
        .service(web::resource("/metrics").route(web::get().to(metrics_handler)))
        .service(web::resource("/").route(web::get().to(test)))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await

}
