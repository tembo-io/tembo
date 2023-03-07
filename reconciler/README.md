# Reconciler

The reconciler is responsible for creating, updating, deleting database instances (custom resource) on a kubernetes cluster.
It runs in each data plane and performs these actions based on messages written to a queue in the control plane.
Upon connecting to this queue, it will continuously poll for new messages posted by the `cp-service` component.

The reconciler will perform the following actions based on `message_type`:
- `Create` or `Update`
  - Create a namespace if it does not already exist.
  - Create an `IngressRouteTCP` object if it does not already exist.
  - Create or update `CoreDB` object.
- `Delete`
  - Delete `CoreDB`.
  - Delete namespace.

Once the reconciler performs these actions, it will send the following information back to a queue from which
`cp-service` will read and flow back up to the UI.

Try running the functional tests locally and then connect into the database to view the structure of these messages.

## Local development

Look in the CI workflow `reconciler-test.yaml` for details on the following.

Prerequisites:
- rust / cargo
- docker
- kind

1. Start a local `kind` cluster

   `❯ kind create cluster`

1. Install CoreDB operator in the cluster
   1. `cargo install coredb-cli`
   2. `coredb-cli install`

1. Install Traefik in the cluster
   1. `helm repo add traefik https://traefik.github.io/charts`
   2. `helm repo update`
   3. `helm install --create-namespace --namespace=traefik-v2 --values ./tests/traefik-values.yaml traefik traefik/traefik`

1. Set up local postgres queue

   `❯ docker run -d --name pgmq -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres`

1. Set the following environment variables:
   - `PG_CONN_URL`
   - `CONTROL_PLANE_EVENTS_QUEUE`
   - `DATA_PLANE_EVENTS_QUEUE`
```
export PG_CONN_URL=postgres://postgres:postgres@localhost:5432/postgres
export CONTROL_PLANE_EVENTS_QUEUE=myqueue_control_plane
export DATA_PLANE_EVENTS_QUEUE=myqueue_data_plane
```

1. Run the reconciler

   `❯ cargo run`

1. Next, you'll need to post some messages to the queue for the reconciler to pick up. That can be performed in functional testing like this `cargo test -- --ignored`
