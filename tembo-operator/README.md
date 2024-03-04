# Tembo Operator

The Tembo Operator is a Kubernetes Operator used for creating, updating, and deleting Postgres instances.

The key differentiators of the Tembo Operator are:

- Unique extension management experience
- Concept of Stacks
- Easily integrate Apps to run alongside Postgres

## Table of Contents

1. [Quick start](#quick-started)
    1. [Cluster operations](#cluster-operations)
2. [Examples](#examples)
    1. [Trying out Postgres extensions](#trying-out-postgres-extensions)
    2. [Trying out Tembo Stacks](#trying-out-tembo-stacks) 
3. [Observability with OpenTelemetry and Jaeger](#observability-with-opentelemetry-and-jaeger)

## Quick Start

While it's fairly straightforward to get started running the Tembo Operator locally, there are a number of prerequisite software that you'll need before getting started.
For an exhaustive list, please refer to the project's [contributing guide](./CONTRIBUTING.md).

### Cluster Operation

To destroy any existing cluster and start the containers needed for a new one, simply run:

```bash
just start-kind
```

In addition to starting the cluster, a fair number of necessary dependencies are installed by subtasks. Check out the definition in the `justfile` if you're curious, it's all pretty composable.

Once the `kind` cluster has been started, you can start a local copy of the Tembo Operator to use it. Again, it's pretty easy:

```bash
just run
```

### Install on an existing cluster

```bash
just install-depedencies
just install-chart
```

#### Integration testing


Or, you can follow the below steps.

- Connect to a cluster that is safe to run the tests against
- Set your kubecontext to any namespace, and label it to indicate it is safe to run tests against this cluster (do not do against non-test clusters)

```bash
NAMESPACE=<namespace> just annotate
```

- Start or install the controller you want to test (see the following sections), do this in a separate shell from where you will run the tests

```
export DATA_PLANE_BASEDOMAIN=localhost
cargo run
```

- Run the integration tests
  > Note: for integration tests to work you will need to be logged in on `plat-dev` via CLI under the "PowerUserAccess" role found here: https://d-9067aa6f32.awsapps.com/start (click "Command line or programmatic access")

```bash
cargo test -- --ignored
```

- The integration tests assume you already have installed or are running the operator connected to the cluster.

#### Other testing notes

- Include the `--nocapture` flag to show print statements during test runs

### Cluster

As an example; install [`kind`](https://kind.sigs.k8s.io/docs/user/quick-start/#installation). Once installed, follow [these instructions](https://kind.sigs.k8s.io/docs/user/local-registry/) to create a kind cluster connected to a local image registry.

### CRD

Apply the CRD from [cached file](charts/coredb-operator/templates/crd.yaml), or pipe it from `crdgen` (best if changing it):

```sh
just install-crd
```

## Observability with OpenTelemetry and Jaeger

[OpenTelemetry](https://opentelemetry.io/) is an observability framework that focuses on generation, collection, management, and export of telemetry.
By integrating it in the Tembo Operator, users are able to gain more insights into their operations.

### Starting out

If you haven't already, you can start a local Kubernetes cluster by running the following:

```bash
just start-kind
```

Once complete, simply run:

```bash
just run-telemetry
```

You're all set to visit the below URL and navigate your telemetry:
```
http://localhost:16686
```

Setup an OpenTelemetry Collector in your cluster. [Tempo](https://github.com/grafana/helm-charts/tree/main/charts/tempo) / [opentelemetry-operator](https://github.com/open-telemetry/opentelemetry-helm-charts/tree/main/charts/opentelemetry-operator) / [grafana agent](https://github.com/grafana/helm-charts/tree/main/charts/agent-operator) should all work out of the box. If your collector does not support grpc otlp you need to change the exporter in [`main.rs`](./src/main.rs).

## Running

### Locally

```sh
cargo run
```

- Or, with optional telemetry (change as per requirements):

```sh
OPENTELEMETRY_ENDPOINT_URL=https://0.0.0.0:55680 RUST_LOG=info,kube=trace,controller=debug cargo run --features=telemetry
```

## Usage

In either of the run scenarios, your app is listening on port `8080`, and it will observe events.

Try some of:

```sh
kubectl apply -f yaml/sample-coredb.yaml
kubectl delete coredb sample-coredb
kubectl edit coredb sample-coredb # change replicas
```

The reconciler will run and write the status object on every change. You should see results in the logs of the pod, or on the .status object outputs of `kubectl get coredb -o yaml`.

### Webapp output

The sample web server exposes some example metrics and debug information you can inspect with `curl`.

```sh
$ kubectl apply -f yaml/sample-coredb.yaml
$ curl 0.0.0.0:8080/metrics
# HELP cdb_controller_reconcile_duration_seconds The duration of reconcile to complete in seconds
# TYPE cdb_controller_reconcile_duration_seconds histogram
cdb_controller_reconcile_duration_seconds_bucket{le="0.01"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="0.1"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="0.25"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="0.5"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="1"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="5"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="15"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="60"} 1
cdb_controller_reconcile_duration_seconds_bucket{le="+Inf"} 1
cdb_controller_reconcile_duration_seconds_sum 0.013
cdb_controller_reconcile_duration_seconds_count 1
# HELP cdb_controller_reconciliation_errors_total reconciliation errors
# TYPE cdb_controller_reconciliation_errors_total counter
cdb_controller_reconciliation_errors_total 0
# HELP cdb_controller_reconciliations_total reconciliations
# TYPE cdb_controller_reconciliations_total counter
cdb_controller_reconciliations_total 1
$ curl 0.0.0.0:8080/
{"last_event":"2019-07-17T22:31:37.591320068Z"}
```

The metrics will be auto-scraped if you have a standard [`PodMonitor` for `prometheus.io/scrape`](https://github.com/prometheus-community/helm-charts/blob/b69e89e73326e8b504102a75d668dc4351fcdb78/charts/prometheus/values.yaml#L1608-L1650).
