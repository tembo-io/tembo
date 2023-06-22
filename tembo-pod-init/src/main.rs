use actix_web::{dev::ServerHandle, middleware, web, App, HttpServer};
use kube::Client;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use parking_lot::Mutex;
//use std::sync::Arc;
use tembo_pod_init::{config::Config, health::*, log, mutate::mutate, watcher::NamespaceWatcher};

#[macro_use]
extern crate tracing;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Config::default();

    // Initialize logging
    if let Err(e) = log::init(&config) {
        error!("Failed to initialize logging: {}", e);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Logger initialization failed",
        ));
    }

    let stop_handle = web::Data::new(StopHandle::default());

    // Setup Kubernetes Client
    let kube_client = match Client::try_default().await {
        Ok(client) => client,
        Err(e) => {
            panic!("Failed to create Kubernetes client: {}", e);
        }
    };

    // Start watching namespaces in a seperate tokio task thread
    let watcher = NamespaceWatcher::new(kube_client.clone(), config.clone());
    let namespaces = watcher.get_namespaces();
    tokio::spawn(async move {
        loop {
            match watcher.watch().await {
                Ok(_) => break,
                Err(e) => {
                    error!("Namespace watcher failed: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    });

    // Print out the namespaces we are currently watching every 10 seconds
    //if tracing::Level::DEBUG >= *tracing::level_filters::RECORDED_LEVEL {
    //    let debug_namespaces = Arc::clone(&namespaces);
    //    tokio::spawn(async move {
    //        loop {
    //            let stored_namespaces = debug_namespaces.read().await;
    //            debug!(
    //                "Namespaces currently being tracked: {:?}",
    //                *stored_namespaces
    //            );
    //            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await; // adjust the delay as needed
    //        }
    //    });
    //}

    // Load the TLS certificate and key
    let mut tls_config = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    tls_config
        .set_private_key_file(config.tls_key.clone(), SslFiletype::PEM)
        .unwrap();
    tls_config
        .set_certificate_chain_file(config.tls_cert.clone())
        .unwrap();
    let server_bind_address = format!("{}:{}", config.server_host, config.server_port);

    let server = HttpServer::new({
        let config_data = web::Data::new(config.clone());
        let kube_data = web::Data::new(kube_client.clone());
        let namespace_watcher_data = web::Data::new(namespaces.clone());
        let stop_handle = stop_handle.clone();
        move || {
            {
                App::new()
                    .app_data(config_data.clone())
                    .app_data(kube_data.clone())
                    .app_data(namespace_watcher_data.clone())
                    .app_data(stop_handle.clone())
                    .wrap(
                        middleware::Logger::default()
                            .exclude("/health/liveness")
                            .exclude("/health/readiness"),
                    )
                    .service(liveness)
                    .service(readiness)
                    .service(mutate)
            }
        }
    })
    .bind_openssl(server_bind_address, tls_config)?
    .shutdown_timeout(5)
    .run();

    stop_handle.register(server.handle());

    info!(
        "Starting HTTPS server at https://{}:{}/",
        config.server_host, config.server_port
    );
    debug!("Config: {:?}", config);
    server.await
}

#[derive(Default)]
struct StopHandle {
    inner: Mutex<Option<ServerHandle>>,
}

impl StopHandle {
    // Set the ServerHandle to stop
    pub(crate) fn register(&self, handle: ServerHandle) {
        *self.inner.lock() = Some(handle);
    }
}
