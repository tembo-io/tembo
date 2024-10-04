# Conductor


Dataplanes receive the desired state of databases from the control plane. The dataplanes report back the actual status of database to the control plane.

Conductor is responsible for:

- Dequeuing desired state from control plane, and applying into Kubernetes
- Enqueuing back to the control plane the actual state of resources

## Local development

Local development involves running Kubernetes and a Postgres container locally inside Docker. Conductor runs with 'cargo run' connected to both the local cluster and database while in development and for functional testing.

Prerequisites:
- rust / cargo
- docker
- [helm](https://helm.sh/docs/intro/install/)
- [kind](https://kind.sigs.k8s.io/)
- [just](https://github.com/casey/just)
- [cargo-watch](https://crates.io/crates/cargo-watch)

1. Start a local `kind` cluster

   `❯ just start-kind`

3. Set up local postgres instance for the queue

   `❯ just run-postgres`

4. Run & watch Conductor

   `❯ just watch`

6. Run unit and functional tests

   `❯ just run-tests`
