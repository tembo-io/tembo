## CoreDB Operator

A rust kubernetes controller for a [`CoreDB` resource](https://github.com/CoreDB-io/coredb/blob/main/coredb-operator/yaml/crd.yaml) using [kube-rs](https://github.com/kube-rs/kube-rs/).

The `Controller` object reconciles `CoreDB` instances when changes to it are detected, writes to its .status object, creates associated events, and uses finalizers for guaranteed delete handling.

## Requirements
- A Kubernetes cluster
- The [CRD](yaml/crd.yaml)
- Opentelemetry collector (**optional**)

### Linting

Run linting with `cargo fmt` and `clippy`

Clippy:
```
rustup component add clippy
cargo clippy
```

cargo fmt:
```
rustup component add rustfmt --toolchain nightly
cargo +nightly fmt
```

### Testing


#### Unit testing

```
cargo test
```

#### Integration testing

- Connect to a cluster that is safe to run the tests against
- Set your kubecontext to any namespace, and label it to indicate it is safe to run tests against this cluster (do not do against non-test clusters)
```
kubectl label namespace default safe-to-run-coredb-tests=true
```

- Start or install the controller you want to test (see the following sections)
- Run the integration tests
```
cargo test -- --ignored
```
- The integration tests assume you already have installed or are running the operator connected to the cluster.

#### Other testing notes

- Include the `--nocapture` flag to show print statements during test runs

### Cluster
As an example; install [`kind`](https://kind.sigs.k8s.io/docs/user/quick-start/#installation). Once installed, follow [these instructions](https://kind.sigs.k8s.io/docs/user/local-registry/) to create a kind cluster connected to a local image registry.

### CRD
Apply the CRD from [cached file](yaml/crd.yaml), or pipe it from `crdgen` (best if changing it):

```sh
cargo run --bin crdgen | kubectl apply -f -
```

### Opentelemetry (optional)
Setup an opentelemetry collector in your cluster. [Tempo](https://github.com/grafana/helm-charts/tree/main/charts/tempo) / [opentelemetry-operator](https://github.com/open-telemetry/opentelemetry-helm-charts/tree/main/charts/opentelemetry-operator) / [grafana agent](https://github.com/grafana/helm-charts/tree/main/charts/agent-operator) should all work out of the box. If your collector does not support grpc otlp you need to change the exporter in [`main.rs`](./src/main.rs).

## Running

### Locally

```sh
cargo run
```

- Or, you can run with auto-reloading your local changes.
- First, install cargo-watch
```
cargo install cargo-watch
```
- Then, run with auto-reload
```
cargo watch -x 'run'
```

- Or, with optional telemetry (change as per requirements):

```sh
OPENTELEMETRY_ENDPOINT_URL=https://0.0.0.0:55680 RUST_LOG=info,kube=trace,controller=debug cargo run --features=telemetry
```

### In-cluster
Compile the controller with:
```sh
just compile
```

Build an image with:
```sh
just build
```

Push the image to your local registry with:
```sh
docker push localhost:5001/controller:<tag>
```

Edit the [deployment](./yaml/deployment.yaml)'s image tag appropriately, then run:

```sh
kubectl apply -f yaml/deployment.yaml
kubectl port-forward service/coredb-controller 8080:80
```

**NB**: namespace is assumed to be `default`. If you need a different namespace, you can replace `default` with whatever you want in the yaml and set the namespace in your current-context to get all the commands here to work.

## Usage
In either of the run scenarios, your app is listening on port `8080`, and it will observe `CoreDB` events.

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
