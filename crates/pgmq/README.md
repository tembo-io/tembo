# Postgres Message Queue (PGMQ)

A lightweight distributed message queue for Rust. Like [AWS SQS](https://aws.amazon.com/sqs/) and [RSMQ](https://github.com/smrchy/rsmq) but on Postgres.

Not building in Rust? Try the [CoreDB pgmq extension](https://github.com/CoreDB-io/coredb/tree/main/extensions/pgx_pgmq).

## Features

- Lightweight - Rust and Postgres only
- Guaranteed delivery of messages to exactly one consumer within a visibility timeout
- API parity with [AWS SQS](https://aws.amazon.com/sqs/) and [RSMQ](https://github.com/smrchy/rsmq)
- Messages stay in the queue until deleted
- Messages can be archived, instead of deleted, for long-term retention and replayability
- Completely asynchronous API

## Quick start

- First, you will need Postgres. We use a container in this example.

```bash
docker run -d --name postgres -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres
```

- If you don't have Docker installed, it can be found [here](https://docs.docker.com/get-docker/).

- Make sure you have the Rust toolchain installed:

```bash
cargo --version
```

- This example was written with version 1.67.0, but the latest stable should work. You can go [here](https://www.rust-lang.org/tools/install) to install Rust if you don't have it already, then run `rustup install stable` to install the latest, stable toolchain.

- Next, let's create a Rust project for the demo.

```bash
# Create a new Rust project
cargo new basic

# Change directory into the new project
cd basic
```

- Add PGMQ to the project

```
cargo add pgmq
```

- Add other dependencies to the project

```
cargo add tokio serde serde_json
```

- Replace the contents of `src/main.rs` with this:

```rust
use pgmq::{Message, PGMQueue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[tokio::main]
async fn main() {

    // Initialize a connection to Postgres
    println!("Connecting to Postgres");
    let queue: PGMQueue = PGMQueue::new("postgres://postgres:postgres@0.0.0.0:5432".to_owned())
        .await
        .expect("failed to connect to postgres");

    // Create a queue
    println!("Creating a queue 'my_queue'");
    let my_queue = "my_queue".to_owned();
    queue.create(&my_queue)
        .await
        .expect("Failed to create queue");

    // Send a message as JSON
    let json_message = serde_json::json!({
        "foo": "bar"
    });
    println!("Enqueueing a JSON message: {json_message}");
    let json_message_id: i64 = queue
        .send(&my_queue, &json_message)
        .await
        .expect("Failed to enqueue message");

    // Messages can also be sent from structs
    #[derive(Serialize, Debug, Deserialize)]
    struct MyMessage {
        foo: String,
    }
    let struct_message = MyMessage {
        foo: "bar".to_owned(),
    };
    println!("Enqueueing a struct message: {:?}", struct_message);
    let struct_message_id: i64 = queue
        .send(&my_queue, &struct_message)
        .await
        .expect("Failed to enqueue message");

    // Use a visibility timeout of 30 seconds.
    //
    // Messages that are not deleted within the
    // visilibity timeout will return to the queue.
    let visibility_timeout_seconds: i32 = 30;

    // Read the JSON message
    let received_json_message: Message<Value> = queue
        .read::<Value>(&my_queue, Some(&visibility_timeout_seconds))
        .await
        .unwrap()
        .expect("No messages in the queue");
    println!("Received a message: {:?}", received_json_message);

    // Compare message IDs
    assert_eq!(received_json_message.msg_id, json_message_id);

    // Read the struct message
    let received_struct_message: Message<MyMessage> = queue
        .read::<MyMessage>(&my_queue, Some(&visibility_timeout_seconds))
        .await
        .unwrap()
        .expect("No messages in the queue");
    println!("Received a message: {:?}", received_struct_message);

    assert_eq!(received_struct_message.msg_id, struct_message_id);

    // Delete the messages to remove them from the queue
    let _ = queue.delete(&my_queue, &received_json_message.msg_id)
        .await
        .expect("Failed to delete message");
    let _ = queue.delete(&my_queue, &received_struct_message.msg_id)
        .await
        .expect("Failed to delete message");
    println!("Deleted the messages from the queue");

    // No messages are remaining
    let no_message: Option<Message<Value>> = queue.read::<Value>(&my_queue, Some(&visibility_timeout_seconds))
        .await
        .unwrap();
    assert!(no_message.is_none());
}
```

- Run the program

- This example is present in the examples/basic directory

```
cargo run
```

## Sending messages

`queue.send()` can be passed any type that implements `serde::Serialize`. This means you can prepare your messages as JSON or as a struct.

## Reading messages

Reading a message will make it invisible (unavailable for consumption) for the duration of the visibility timeout (vt).
No messages are returned when the queue is empty or all messages are invisible.

Messages can be parsed as serde_json::Value or into a struct. `queue.read()` returns an `Result<Option<Message<T>>, PGMQError>`
where `T` is the type of the message on the queue. It returns an error when there is an issue parsing the message or if PGMQ is unable to reach postgres.
Note that when parsing into a `struct`, the operation will return an error if
parsed as the type specified. For example, if the message expected is
`MyMessage{foo: "bar"}` but` {"hello": "world"}` is received, the application will panic.

## Archive or Delete a message

Remove the message from the queue when you are done with it. You can either completely `.delete()`, or `.archive()` the message. Archived messages are deleted from the queue and inserted to the queue's archive table. Deleted messages are just deleted.

License: MIT
