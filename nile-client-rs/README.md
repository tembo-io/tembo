# Nile API Rust Client

This is a Rust client for the [Nile API](https://thenile.dev/).



## Usage



```rust
use std::env;

use dotenv::dotenv;
use nile_client_rs::{InstanceUpdate, NileClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let username = env::var("NILE_DEVELOPER_EMAIL").expect("NILE_DEVELOPER_EMAIL must be set");
    let password = env::var("NILE_DEVELOPER_PASSWORD").expect("NILE_DEVELOPER_PASSWORD must be set");
    let workspace = env::var("NILE_WORKSPACE").expect("NILE_WORKSPACE must be set");
    let entity_name = env::var("NILE_ENTITY_NAME").expect("NILE_ENTITY_NAME must be set");
    let org = env::var("NILE_ORGANIZATION_NAME").expect("NILE_ORGANIZATION_NAME must be set");


    let mut client = NileClient::default();
    client
        .authenticate(username, password)
        .await?;
```

### List all instances of an entity in a workspace

```rust
let instances = client.get_instances(&workspace, &entity_name).await?;
println!("instances: {:#?}", instances);
```


### Poll for all events for an entity in a workspace
```rust
let events = client.get_events(&workspace, &entity_name, 0, 20).await?;
println!("events: {:#?}", events);
```

### Update an atrribute on an existing instance
```rust
let mut updates = Vec::new();

// update the number of pods
let pod_ct = InstanceUpdate {
    op: "replace".to_owned(),
    path: "/numPods".to_owned(),
    value: "5".to_owned(),
};
updates.push(pod_ct);

// update database status
let status_update = InstanceUpdate {
    op: "replace".to_owned(),
    path: "/status".to_owned(),
    value: "Up".to_owned(),
};
updates.push(status_update);

// send the updates to the Nile
let status = client
    .patch_instance(&workspace, &org, &entity_name, &instance_id, updates)
    .await?;
print!("status: {:#?}", status);
}

```