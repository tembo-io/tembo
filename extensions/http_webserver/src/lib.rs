use pgx::bgworkers::*;
use pgx::log;
use pgx::prelude::*;
use std::io;
use std::net::TcpListener;
use std::panic;


pgx::pg_module_magic!();

#[path = "router.rs"]
mod router;


extension_sql!(
    r#"
CREATE TABLE items (
    id serial8 not null primary key,
    title text
);
INSERT INTO items (title) VALUES ('inserted on init');
"#,
    name = "create_items_table",
);

#[allow(non_snake_case)]
#[pg_guard]
pub extern "C" fn _PG_init() {
    log!("Initializing background worker");
    BackgroundWorkerBuilder::new("Background Worker Example")
        .set_function("background_worker_main")
        .set_library("api")
        .enable_spi_access()
        .set_start_time(BgWorkerStartTime::ConsistentState)
        .load();
}

#[pg_guard]
#[no_mangle]
pub extern "C" fn background_worker_main(_arg: pg_sys::Datum) {
    // these are the signals we want to receive.  If we don't attach the SIGTERM handler, then
    // we'll never be able to exit via an external notification
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    // we want to be able to use SPI against the specified database (postgres), as the superuser which
    // did the initdb. You can specify a specific user with Some("my_user")
    BackgroundWorker::connect_worker_to_spi(Some("api"), None);

    log!(
        "Hello from inside the {} BGWorker!",
        BackgroundWorker::get_name()
    );

    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    listener
        .set_nonblocking(true)
        .expect("Cannot set non-blocking");

    log!("Listening on port 8080");
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                // handle the connection
                let res = panic::catch_unwind(|| router::handle_request(s));
                if res.is_err() {
                    log!("error: {:?}", res);
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if BackgroundWorker::sigterm_received() {
                    // on SIGTERM, we should exit
                    log!("SIGTERM received, exiting");
                    break;
                }

                // otherwise, continue
                continue;
            }
            Err(e) => panic!("encountered IO error: {}", e),
        }
    }

    log!(
        "Goodbye from inside the {} BGWorker! ",
        BackgroundWorker::get_name()
    );
}
