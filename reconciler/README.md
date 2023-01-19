# Reconciler

The reconciler is responsible for creating, updating, deleting database instances (custom resource) on a kubernetes cluster.
It runs in each data plane and performs these actions based on messages written to a queue in the control plane.
Upon connecting to this queue, it will continuously poll for new messages posted by the `cp-service` component. 
These messages are expected to be in the following format:
```json
{
    "body": {
      "cpu": "1",
      "memory": "2Gi",
      "postgres_image": "registry.developers.crunchydata.com/crunchydata/crunchy-postgres:ubi8-14.6-2",
      "resource_name": "example",
      "resource_type": "CoreDB",
      "storage": "1Gi"
    },
    "data_plane_id": "org_02s3owPQskuGXHE8vYsGSY",
    "event_id": "coredb-poc1.org_02s3owPQskuGXHE8vYsGSY.CoreDB.inst_02s4UKVbRy34SAYVSwZq2H",
    "message_type": "Create"
}
```

The reconciler will perform the following actions based on `message_type`:
- `Create` or `Update`
  - Create a namespace if it does not already exist.
  - Create an `IngressRouteTCP` object if it does not already exist.
  - Create or update `PostgresCluster` object.
- `Delete`
  - Delete `PostgresCluster`.
  - Delete namespace.

Once the reconciler performs these actions, it will send the following information back to a queue from which
`cp-service` will read and flow back up to the UI:
```json
{
  "data_plane_id": "org_02s3owPQskuGXHE8vYsGSY",
  "event_id": "coredb-poc1.org_02s3owPQskuGXHE8vYsGSY.CoreDB.inst_02s4UKVbRy34SAYVSwZq2H",
  "event_meta": {
    "connection": "postgresql://example:password@example.coredb-development.com:5432"
  }
}
```

## Local development
Prerequisites:
- rust / cargo
- docker
- kind

1. Start a local `kind` cluster

   `❯ kind create cluster`


1. Install Crunchy PGO on the cluster
   1. Fork and clone https://github.com/CrunchyData/postgres-operator-examples
   2. `❯ cd postgres-operator-examples`
   3. `❯ kubectl apply -k kustomize/install/namespace`
   4. `❯ kubectl apply --server-side -k kustomize/install/default`


1. Set up local postgres queue

   `❯ docker run -d --name pgmq -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres`


1. Set the following environment variables:
   - `PG_CONN_URL`
   - `CONTROL_PLANE_EVENTS_QUEUE`
   - `DATA_PLANE_EVENTS_QUEUE`


1. Run the reconciler

   `❯ cargo run`


1. Next, you'll need to post some messages to the queue for the reconciler to pick up. This could be performed a number of ways.
   The following is an example rust application that writes a message to a queue.

    Run `❯ cargo run` to post the message to the given queue. Edit the message and repeat the process for development purposes.

```rust
use pgmq::{PGMQueue};

#[tokio::main]
async fn run() -> Result<(), sqlx::Error> {
    let queue: PGMQueue =
        PGMQueue::new("postgres://postgres:postgres@0.0.0.0:5432".to_owned()).await;

    let myqueue = "myqueue".to_owned();
    queue.create(&myqueue).await?;


    let msg = serde_json::json!({
    "body": {
      "cpu": "1",
      "memory": "2Gi",
      "postgres_image": "registry.developers.crunchydata.com/crunchydata/crunchy-postgres:ubi8-14.6-2",
      "resource_name": "example",
      "resource_type": "CoreDB",
      "storage": "1Gi"
    },
    "data_plane_id": "org_02s3owPQskuGXHE8vYsGSY",
    "event_id": "coredb-poc1.org_02s3owPQskuGXHE8vYsGSY.CoreDB.inst_02s4UKVbRy34SAYVSwZq2H",
    "message_type": "Create"
});
    let msg_id = queue.enqueue(&myqueue, &msg).await;
    println!("msg_id: {:?}", msg_id);

    Ok(())
}

fn main() {
    run().unwrap();
}
```
