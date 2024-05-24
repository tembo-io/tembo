![tembo](https://github.com/tembo-io/tembo/assets/4283/f9ba2331-dc24-476c-8f83-05d620b66b06)

[![License](https://img.shields.io/badge/license-PostgreSQL-blue)](https://github.com/tembo-io/tembo/blob/main/LICENSE)
[![OSSRank](https://shields.io/endpoint?url=https://ossrank.com/shield/3811)](https://ossrank.com/p/3811)
[![Static Badge](https://img.shields.io/badge/%40tembo-community?logo=slack&label=slack)](https://join.slack.com/t/tembocommunity/shared_invite/zt-293gc1k0k-3K8z~eKW1SEIfrqEI~5_yw)

Tembo improves the developer experience of deploying, managing, and scaling Postgres. Tembo is under active development and you are free to use anything we have open-sourced.

## Why Postgres?

Postgres is the best OSS database in the world, with millions of active deployments, growing faster than MySQL. It is battle-tested with a large community that can handle SQL (relational) and JSON (non-relational) queries and a wide range of workloads (analytical, time-series, geospatial, etc.), through its rich ecosystem of add-ons and extensions.

## Inside this repo

* [Tembo Operator](https://github.com/tembo-io/tembo/tree/main/tembo-operator) - a Kubernetes Operator that integrates CloudNativePG, Tembo Stacks, and Trunk
* [Tembo Stacks](https://github.com/tembo-io/tembo/tree/main/tembo-stacks) - workload-configured Postgres deployable to Kubernetes
* [Tembo CLI](https://github.com/tembo-io/tembo/tree/main/tembo-cli) - allows users to experience Tembo locally, as well as, manage and deploy to Tembo Cloud
* [Tembo Helm Chart](https://github.com/tembo-io/tembo/tree/main/charts/tembo-operator) — Helm chart to deploy the Tembo Operator
* [Tembo Dataplane Web Server](https://github.com/tembo-io/tembo/tree/main/dataplane-webserver) - reports readiness and liviness of Postgres instances in a data plane
* [Tembo Pod Init](https://github.com/tembo-io/tembo/tree/main/tembo-pod-init) - allows us to bootstrap the folder structure needed to add our required mutability
* [Tembo Conductor](https://github.com/tembo-io/tembo/tree/main/conductor) - runs in the dataplane; receive desired states from control plane and reports back status
* [Tembo LLM Inference Server](https://github.com/tembo-io/tembo/tree/main/inference-server) - a LLM hosting service that is built on top of [vLLM](https://github.com/vllm-project/vllm) with usage tracking

## Our other open-source projects 

* [Tembo Terraform Provider](https://github.com/tembo-io/terraform-provider-tembo) - The Terraform provider for Tembo
* [Tembo Telemetry](https://github.com/tembo-io/tembo-telemetry) - Logging and Telemetry exporters for Tembo applications

### Trunk

* [Trunk CLI](https://github.com/tembo-io/trunk/tree/main/cli) that users can use to publish and install Postgres extensions
* [Trunk Website](https://github.com/tembo-io/trunk/tree/main/registry) that serves as a backend for [pgt.dev](https://pgt.dev), and also provides discovery and metrics

### Postgres Extensions

* [pgmq](https://github.com/tembo-io/pgmq) - a message queue built with Rust, available as a Postgres extension and Rust crate
* [pg_vectorize](https://github.com/tembo-io/pg_vectorize) - automate vector search workflow, and SQL access to 100+ OSS sentence transformer models
* [pg_later](https://github.com/tembo-io/pg_later) - a Postgres extension for completely asynchronous query execution
* [pg_tier](https://github.com/tembo-io/pg_tier) - a Postgres Extension to enable data tiering to AWS S3
* [pg_timeseries](https://github.com/tembo-io/pg_timeseries) - a Postgres Extension to provide simple and focused time-series tables
* [pg_auto_dw](https://github.com/tembo-io/pg_auto_dw) - An auto data warehouse extension for Postgres

#### Foreign Data Wrapper Extensions

* [prometheus_fdw](https://github.com/tembo-io/prometheus_fdw) - query and move metrics from [Prometheus](https://prometheus.io/) to Postgres
* [clerk_fdw](https://github.com/tembo-io/clerk_fdw) - connect to [Clerk](https://clerk.com/) User and Organization data from Postgres
* [orb_fdw](https://github.com/tembo-io/orb_fdw) - connect your billing data from [Orb](https://www.withorb.com/) to Postgres 

## Tembo Cloud (GA)

Tembo Cloud is a dev-first, fully-extensible, fully-managed, secure, and scalable Postgres service. The managed service will provide a growing ecosystem of easily-installable extensions, allowing you to expand the use cases of Postgres.

Deploy a free-forever hobby Postgres database and install any of more than 200 extensions at [https://cloud.tembo.io](https://cloud.tembo.io).

## Tembo Self Hosted (Alpha)

Tembo Self-Hosted is a self-hosted version of the Tembo Platform that runs in your own Kubernetes cluster. It allows you to benefit from the same features as Tembo Cloud, but with the added control and security of running the software in your own environment.

Tembo Self-Hosted is made up of the same components as Tembo Cloud, but packaged and distributed in a way that allows for easy installation and management. Instead of running in separate Kubernetes clusters, the components run in a single Kubernetes cluster. This keeps your total cost of ownership low and makes for a simple and easy-to-manage deployment.

If you're interested in using Tembo Self Hosted, [reach out for a license](https://calendly.com/ian-tembo).
