use pgx::bgworkers::*;
use pgx::log;
use pgx::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;

pub fn handle_index() -> String {
    // set up response header
    let response = "HTTP/1.1 200 OK\r\n\r\n";

    // need to use arc + mutex here because of rust ownership rules
    let objects = Arc::new(Mutex::new(vec![]));
    let clone = Arc::clone(&objects);

    // everything must be done in a transaction
    BackgroundWorker::transaction(move || {
        Spi::execute(|client| {
            // select everything from the spi_example table
            let tuple_table = client.select("SELECT id, title FROM items;", None, None);
            log!("ran select query");

            // iterate over the results and add them to the vector
            tuple_table.for_each(|tuple| {
                // we know we're going to have an id and a title
                let id = tuple.by_name("id").unwrap().value::<i64>().unwrap();
                let title = tuple.by_name("title").unwrap().value::<String>().unwrap();

                // add the object to the vector
                let mut obj_clone = clone.lock().unwrap();
                obj_clone.push(format!("{}: {}", id, title));
            });
        });
    });

    // append the objects to the response and return it
    format!("{}\n{}", response, objects.lock().unwrap().join("\n"))
}

pub fn handle_echo(body: &str) -> String {
    let response = "HTTP/1.1 200 OK\r\n\r\n";

    format!("{}\n{}", response, body)
}

#[derive(serde::Deserialize)]
struct AddBody {
    title: String,
}

pub fn handle_add(body: &str) -> String {
    let response = "HTTP/1.1 200 OK\r\n\r\n";

    // first, try to parse the body into a struct. if it fails, return a 400
    let add_body: AddBody = match serde_json::from_str(body) {
        Ok(body) => body,
        Err(_) => {
            return "HTTP/1.1 400 Bad Request\r\n\r\n".to_string();
        }
    };

    // need to use arc + mutex here because of rust ownership rules
    let id = Arc::new(Mutex::new(0));
    let clone = Arc::clone(&id);

    BackgroundWorker::transaction(move || {
        Spi::execute(|client| {
            // generate the query
            let query = format!(
                "INSERT INTO items (title) VALUES ('{}') RETURNING id;",
                add_body.title,
            );

            // run the query
            let tuple_table = client.update(query.as_str(), None, None).first();
            log!("ran insert query");

            // get the id back
            let id: i64 = tuple_table.get_one().unwrap();

            // get the id of the inserted row
            log!("inserted row with id {}", id);

            // set the id in the arc
            let mut id_clone = clone.lock().unwrap();
            *id_clone = id;
        });
    });

    // append the id to the response and return it
    format!("{}\n{}", response, id.lock().unwrap())
}

pub fn handle_delete(url: &str) -> String {
    let response = "HTTP/1.1 200 OK\r\n\r\n";

    // first, get the id from the url. if it fails, return a 400
    let id = match url.split("/").last() {
        Some(id) => id,
        None => {
            return "HTTP/1.1 400 Bad Request\r\n\r\n".to_string();
        }
    };

    // then, try to parse the id into an i64. if it fails, return a 400
    let id: i64 = match id.parse() {
        Ok(id) => id,
        Err(_) => {
            return "HTTP/1.1 400 Bad Request\r\n\r\n".to_string();
        }
    };

    BackgroundWorker::transaction(move || {
        Spi::execute(|client| {
            // generate and run the query
            let query = format!("DELETE FROM items WHERE id = {};", id,);
            client.update(query.as_str(), None, None).first();

            log!("ran delete query");
        });
    });

    // append the id to the response and return it
    format!("{}\n{}", response, id)
}
