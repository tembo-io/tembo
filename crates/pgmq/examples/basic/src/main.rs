use pgmq::{errors::PgmqError, Message, PGMQueue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), PgmqError> {
    // CREATE A QUEUE
    let queue: PGMQueue =
        PGMQueue::new("postgres://postgres:postgres@0.0.0.0:5432".to_owned()).await?;
    let myqueue = "myqueue".to_owned();
    queue
        .create(&myqueue)
        .await
        .expect("Failed to create queue");

    // SEND A `serde_json::Value` MESSAGE
    let msg1 = serde_json::json!({
        "foo": "bar"
    });
    let msg_id1: i64 = queue
        .send(&myqueue, &msg1)
        .await
        .expect("Failed to enqueue message");
    println!("Message ID: {msg_id1}");

    // SEND A STRUCT
    #[derive(Serialize, Debug, Deserialize)]
    struct MyMessage {
        foo: String,
    }
    let msg2 = MyMessage {
        foo: "bar".to_owned(),
    };
    let msg_id2: i64 = queue
        .send(&myqueue, &msg2)
        .await
        .expect("Failed to enqueue message");
    println!("Message ID: {msg_id2}");

    // READ A MESSAGE as `serde_json::Value`
    let vt: i32 = 30;
    let read_msg1: Message<Value> = queue
        .read::<Value>(&myqueue, Some(&vt))
        .await?
        .expect("no messages in the queue!");
    println!("Message: {read_msg1:?}");

    // READ A MESSAGE as a struct
    let read_msg2: Message<MyMessage> = queue
        .read::<MyMessage>(&myqueue, Some(&vt))
        .await?
        .expect("no messages in the queue!");
    println!("Message: {read_msg2:?}");

    // DELETE A MESSAGE WE SENT
    queue
        .delete(&myqueue, &read_msg1.msg_id)
        .await
        .expect("Failed to delete message");

    // ARCHIVE THE OTHER MESSAGE
    queue
        .archive(&myqueue, &read_msg2.msg_id)
        .await
        .expect("Failed to archive message");

    Ok(())
}
