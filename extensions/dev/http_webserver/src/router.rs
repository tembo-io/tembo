use pgx::log;
use std::{io::prelude::*, net::TcpStream};

#[path = "controllers.rs"]
mod controllers;

// this is called anytime we receive a request
pub fn handle_request(mut stream: TcpStream) {
    log!("Handling request");

    let mut request_vec = Vec::new();

    // loop until there are no more bytes to read
    loop {
        log!("Trying to get next chunk...");
        let mut buf = [0; 4096];
        let bytes_read = stream.read(&mut buf).unwrap();
        log!("Got {} bytes", bytes_read);
        request_vec.extend_from_slice(&buf[..bytes_read]);

        // if we have a full request, break out of the loop

        if bytes_read < 4096 {
            break;
        }
    }

    // turn the request into a string to make it easier to work with
    let request: String = String::from_utf8(request_vec).unwrap();

    // log the request
    log!("got request: \n\n{}\n\n", request);

    // parse the request using httparse
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);
    let status = req.parse(request.as_bytes()).unwrap();

    // get the body
    let body = match status {
        httparse::Status::Complete(len) => {
            let body = &request[len..];
            body
        }
        _ => {
            log!("got incomplete request");
            ""
        }
    };

    // match on the start of the path to determine which controller to use
    let response = || -> String {
        let url = req.path.unwrap();
        if url == "/" {
            controllers::handle_index()
        } else if url.starts_with("/echo") {
            controllers::handle_echo(body)
        } else if url.starts_with("/add") {
            controllers::handle_add(body)
        } else if url.starts_with("/delete") {
            controllers::handle_delete(url)
        } else {
            "HTTP/1.1 404 Not Found\r\n\r\n".to_string()
        }
    }();

    // send the response back to the client
    stream.write_all(response.as_bytes()).unwrap();
}
