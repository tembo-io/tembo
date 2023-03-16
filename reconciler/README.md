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

   `> cargo install coredb-cli`

   `> coredb-cli install --branch main`

2. Install Traefik in the cluster
   
   `> make setup.traefik`

3. Set up local postgres queue

   `❯ docker run -d --name pgmq -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres`

4. Run the reconciler

   `❯ make run.local`

5. Or, run the reconciler with the test configuration:

   `❯ make run.test`

6. Next, you'll need to post some messages to the queue for the reconciler to pick up. That can be performed in functional testing like this `cargo test -- --ignored`.

## Codegen

This project requires generated client side types for the coredb-operator. To update:

Install `kopium` if you don't have it already.

   `> cargo install kopium`

Download the specified CoreDB spec.

   `> wget https://raw.githubusercontent.com/CoreDB-io/coredb/release/2023.3.9/coredb-operator/yaml/crd.yaml`

Generate the Rust code. Note: there are several overrides. Inspect the git diff and adjust accordingly.

   `> kopium -f crd.yaml > src/coredb_crd.rs`

## Integration Testing

1. Ensure operator is running locally (see instructions above)
2. Start the Reconciler using test configuration: `make run.test`
3. In another terminal session, `cargo test -- --ignored`
