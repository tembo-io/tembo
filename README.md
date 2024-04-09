![tembo](https://github.com/tembo-io/tembo/assets/4283/f9ba2331-dc24-476c-8f83-05d620b66b06)

[![License](https://img.shields.io/badge/license-PostgreSQL-blue)](https://github.com/tembo-io/tembo/blob/main/LICENSE)
[![OSSRank](https://shields.io/endpoint?url=https://ossrank.com/shield/3811)](https://ossrank.com/p/3811)
[![Static Badge](https://img.shields.io/badge/%40tembo-community?logo=slack&label=slack)](https://join.slack.com/t/tembocommunity/shared_invite/zt-293gc1k0k-3K8z~eKW1SEIfrqEI~5_yw)

Tembo aims to improve the experience developers have with deploying, managing, and scaling Postgres. Tembo is under active development that you are free to use anything that we have open-sourced.

## Why Postgres?

Postgres is the best OSS database in the world, with millions of active deployments, and is growing faster than MySQL. It is battle-tested with a large community that can handle SQL (relational) and JSON (non-relational) queries and a widerange of workloads (i.e, analytical, time-series, geospatial, etc), on account of its’ rich ecosystem of add-ons and extensions.

## Inside this repo

* [Tembo Operator](https://github.com/tembo-io/tembo/tree/main/tembo-operator) - a Kubernetes Operator that integrates CloudNativePG, Tembo Stacks, and Trunk
* [Tembo Stacks](https://github.com/tembo-io/tembo/tree/main/tembo-stacks) - workload-configured Postgres deployable to Kubernetes
* [Tembo CLI](https://github.com/tembo-io/tembo/tree/main/tembo-cli) - allows users to experience Tembo locally, as well as, manage and deploy to Tembo Cloud
* [Helm Chart](https://github.com/tembo-io/tembo/tree/main/charts/tembo-operator) — Helm chart to deploy the Tembo Operator
* [Dataplane Web Server](https://github.com/tembo-io/tembo/tree/main/dataplane-webserver) - reports readiness and liviness of Postgres instances in a data plane
* [tembo-pod-init](https://github.com/tembo-io/tembo/tree/main/tembo-pod-init) - allows us to bootstrap the folder structure needed to add our required mutability
* [Conductor](https://github.com/tembo-io/tembo/tree/main/conductor) - runs in the dataplane; receive desired states from control plane and reports back status

## Our other open-source projects 

### Trunk

* [Trunk CLI](https://github.com/tembo-io/trunk/tree/main/cli) that users can use to publish and install Postgres extensions
* [Trunk Website](https://github.com/tembo-io/trunk/tree/main/registry) that serves as a backend for [pgt.dev](https://pgt.dev), and also provides discovery and metrics

### Postgres Extensions

* [pgmq](https://github.com/tembo-io/pgmq) - a message queue built with Rust and available as a Postgres extension and Rust crate (used in our managed service)
* [pg_later](https://github.com/tembo-io/pg_later) - a Postgres extension for completely asynchronous query execution
* [pg_vectorize](https://github.com/tembo-io/pg_vectorize) - automate vector search workflow, and SQL access to 100+ OSS sentence transformer models
* [clerk_fdw](https://github.com/tembo-io/clerk_fdw) - connect to [Clerk](https://clerk.com/) User and Organization data from Postgres
* [prometheus_fdw](https://github.com/tembo-io/prometheus_fdw) - query and move metrics from [Prometheus](https://prometheus.io/) to Postgres
* [orb_fdw](https://github.com/tembo-io/orb_fdw) - connect your billing data from [Orb](https://www.withorb.com/) to Postgres 

## Tembo Cloud

Tembo Cloud is a dev-first, fully-extensible, fully-managed, secure, and scalable Postgres service. The managed service will provide a growing ecosystem of easily-installed
extensions, allowing you to expand the use cases of Postgres.

Signup and deploy a free Postgres database with ~200 extensions at [https://cloud.tembo.io](https://cloud.tembo.io).
