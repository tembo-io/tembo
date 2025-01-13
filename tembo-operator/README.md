# Tembo Operator

The Tembo Operator is a Kubernetes Operator used for creating, updating, and deleting PostgreSQL (Postgres) instances.

The key differentiators of the Tembo Operator are:

- Unique extension management experience
- Concept of Stacks
- Easily integrate Apps to run alongside Postgres

## Table of Contents

1. [Quick start](#quick-start)
    1. [Cluster operations](#cluster-operations)
2. [Examples](#examples)
    1. [Trying out Postgres extensions](#1-trying-out-postgres-extensions)
    2. [Trying out Tembo Stacks](#2-trying-out-tembo-stacks)
3. [Observability with curl](#observability-with-curl)
4. [Observability with OpenTelemetry and Jaeger](#observability-with-opentelemetry-and-jaeger)

## Quick Start

While it's fairly straightforward to get started running the Tembo Operator locally, there are prerequisites that you'll need before getting started.
For an exhaustive list, please refer to the project's [contributing guide](./CONTRIBUTING.md).

### Cluster Operations

To destroy any existing cluster and start the containers needed for a new one, simply run:

```bash
just start-kind
```

In addition to starting the cluster, several necessary dependencies are installed by subtasks. Check out the definition in the `justfile` if you're curious, it's all pretty composable.

Once the `kind` cluster has been started, you can start a local copy of the Tembo Operator to use it. Again, it's pretty easy:

```bash
just run
```

## Examples

With the cluster running, you're ready to test some of the built-in features of the Tembo Operator.

### 1. Trying out Postgres Extensions

:bulb: The following steps assume you have gone through the [quick start section](#quick-start).

Start by applying a YAML template of your choosing, hosted at [./yaml](./yaml):

```bash
kubectl apply -f yaml/sample-standard.yaml
```

Once established, `psql` into the pod.
This will require a password, the instructions of which are outlined in the CONTRIBUTING.md file's [Connect via psql](https://github.com/tembo-io/tembo/blob/main/tembo-operator/CONTRIBUTING.md#4-connect-via-psql) section.

```bash
psql postgres://postgres:$PGPASSWORD@sample-standard.localhost:5432
```

This will allow you to see the enabled extensions with:

```sql
\dx
```

As well as the to-be-enabled extensions that have already been installed:

```sql
SELECT * FROM pg_available_extensions;
```

Go ahead and exit Postgres with `\q` and the pod with `Ctl-D`.

Now try to `exec` into the pod via:

```bash
kubectl exec -it sample-standard-1 -- /bin/bash
```

As mentioned above, the Tembo Operator comes pre-packaged with added Postgres extension management.
This comes in the form of the [Trunk](https://pgt.dev/) extension registry, which can be used from within the Kubernetes pod.

Try running the following:

```bash
trunk install pgmq
```

:bulb: Note that you can choose to install any extension found within the Trunk registry.

Considering that you should still be in the pod, simply run `psql` and you should find yourself in the Postgres instance.

As before, try running

```sql
SELECT * FROM pg_available_extensions WHERE name = 'pgmq'
```

You should see the extension in the results, from which you can run to enable:

```sql
CREATE EXTENSION pgmq CASCADE;
```

### 2. Trying out Tembo Stacks

In the above example, we utilize the `sample-standard.yaml` file, but there are others that offer distinct configurations.
Check out the others in the yaml directory.
Here are some select options to apply just as `sample-standard.yaml` was:

- Try out the [Message Queue Stack](https://tembo.io/docs/tembo-stacks/message-queue) with [sample-message-queue.yaml](./yaml/sample-message-queue.yaml).
- Try out the [MongoAlternative Stack](https://tembo.io/docs/tembo-stacks/mongo-alternative) with [sample-document.yaml](./yaml/sample-document.yaml).

## Observability with curl

In either of the above scenarios, your app is listening on port `8080`, and it will observe events.

The reconciler will run and write the status object on every change. You should see results in the logs of the pod, or on the .status object outputs of `kubectl get coredb -o yaml`.

### Webapp Output

The sample web server exposes some example metrics and debug information you can inspect with `curl`.

```bash
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

## Observability with OpenTelemetry and Jaeger

[OpenTelemetry](https://opentelemetry.io/) is an observability framework that focuses on generation, collection, management, and export of telemetry.
[Jaeger](https://www.jaegertracing.io/), on the other hand, is an observability platform with a companion UI that ingests the OpenTelemetry data.
By integrating both into the Tembo Operator, users are able to gain more insights into their operations.

If you haven't already, you can start a local Kubernetes cluster by running the following:

```bash
just start-kind
```

Once complete, simply run:

```bash
just run-telemetry
```

From there, you're all set to visit the below URL and navigate your telemetry:
```
http://localhost:16686
```
