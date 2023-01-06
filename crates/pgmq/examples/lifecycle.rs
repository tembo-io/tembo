use pgmq::{Message, PGMQueue, PGMQueueConfig};

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {

    let queue: PGMQueue = PGMQueue::new("postgres://postgres:postgres@0.0.0.0:5432".to_owned());

    let myqueue = "myqueue".to_owned();
    queue.create(&myqueue).await?;

    let msg = serde_json::json!({
        "foo": "bar"
    });
    let msg_id = queue.enqueue(&myqueue, &msg).await;
    println!("msg_id: {:?}", msg_id);

    let read_msg: Message = queue.read(&myqueue, Some(&30_u32)).await.unwrap();
    print!("read_msg: {:?}", read_msg);

    let deleted = queue.delete(&myqueue, &read_msg.msg_id).await;
    println!("deleted: {:?}", deleted);

    Ok(())
}
