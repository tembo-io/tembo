## CoreDB Operator
[![ci](https://github.com/kube-rs/controller-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/kube-rs/controller-rs/actions/workflows/ci.yml)
[![docker image](https://img.shields.io/docker/pulls/clux/controller.svg)](
https://hub.docker.com/r/clux/controller/tags/)

A rust kubernetes reference controller for a [`CoreDB` resource](https://github.com/kube-rs/controller-rs/blob/master/yaml/crd.yaml) using [kube-rs](https://github.com/kube-rs/kube-rs/), with observability instrumentation.

The `Controller` object reconciles `CoreDB` instances when changes to it are detected, writes to its .status object, creates associated events, and uses finalizers for guaranteed delete handling.

## Requirements
- A Kubernetes cluster / k3d instance
- The [CRD](yaml/crd.yaml)
- Opentelemetry collector (**optional**)

### Cluster
As an example; get `k3d` then:

```sh
k3d cluster create --registry-create --servers 1 --agents 1 main
k3d kubeconfig get --all > ~/.kube/k3d
export KUBECONFIG="$HOME/.kube/k3d"
```

A default `k3d` setup is fastest for local dev due to its local registry.

### CRD
Apply the CRD from [cached file](yaml/crd.yaml), or pipe it from `crdgen` (best if changing it):

```sh
cargo run --bin crdgen | kubectl apply -f -
```

### Opentelemetry
Setup an opentelemetry collector in your cluster. [Tempo](https://github.com/grafana/helm-charts/tree/main/charts/tempo) / [opentelemetry-operator](https://github.com/open-telemetry/opentelemetry-helm-charts/tree/main/charts/opentelemetry-operator) / [grafana agent](https://github.com/grafana/helm-charts/tree/main/charts/agent-operator) should all work out of the box. If your collector does not support grpc otlp you need to change the exporter in [`main.rs`](./src/main.rs).

If you don't have a collector, you can build locally without the `telemetry` feature (`tilt up telemetry`), or pull images [without the `otel` tag](https://hub.docker.com/r/clux/controller/tags/).

## Running

### Locally

```sh
cargo run
```

or, with optional telemetry (change as per requirements):

```sh
OPENTELEMETRY_ENDPOINT_URL=https://0.0.0.0:55680 RUST_LOG=info,kube=trace,controller=debug cargo run --features=telemetry
```

### In-cluster
Use either your locally built image or the one from dockerhub (using opentemetry features by default). Edit the [deployment](./yaml/deployment.yaml)'s image tag appropriately, and then:

```sh
kubectl apply -f yaml/deployment.yaml
kubectl wait --for=condition=available deploy/coredb-controller --timeout=20s
kubectl port-forward service/coredb-controller 8080:80
```

To build and deploy the image quickly, we recommend using [tilt](https://tilt.dev/), via `tilt up` instead.

**NB**: namespace is assumed to be `default`. If you need a different namespace, you can replace `default` with whatever you want in the yaml and set the namespace in your current-context to get all the commands here to work.

## Usage
In either of the run scenarios, your app is listening on port `8080`, and it will observe `CoreDB` events.

Try some of:

```sh
kubectl apply -f yaml/sample-coredb.yaml
kubectl delete coredb sample-coredb
kubectl edit coredb sample-coredb # change hidden
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

### Events
The example `reconciler` only checks the `.spec.hidden` bool. If it does, it updates the `.status` object to reflect whether or not the instance `is_hidden`. It also sends a kubernetes event associated with the controller. It is visible at the bottom of `kubectl describe coredb sample-coredb`.

While this controller has no child objects configured, there is a [`configmapgen_controller`](https://github.com/kube-rs/kube-rs/blob/master/examples/configmapgen_controller.rs) example in [kube-rs](https://github.com/kube-rs/kube-rs/).
