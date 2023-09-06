[![conductor-deploy workflow](https://github.com/tembo-io/data-plane/actions/workflows/deploy.yml/badge.svg?branch=main)](https://github.com/CoreDB-io/data-plane/actions/workflows/deploy.yml)
[![OSSRank](https://shields.io/endpoint?url=https://ossrank.com/shield/3811)](https://ossrank.com/p/3811)

# Tembo Stacks

Goodbye database sprawl, hello Postgres.

Deploy data services on PostgreSQL with **the Tembo Operator for Kubernetes** and **Tembo Stacks**, which are packages of community and purpose-built extensions to customize PostgreSQL for a variety of use cases.

This code repository is our entire "data plane" codebase. You can use it to self-host Tembo Stacks in your own Kubernetes cluster.

## Current stacks

- Standard
- OLTP
- OLAP
- Enterprise LLM
- Messaging

## Try on Tembo Cloud

You can sign up for Tembo Cloud at [cloud.tembo.io](https://cloud.tembo.io).

Tembo Cloud is a managed service where users can deploy Postgres in various forms. We have a control plane / data plane architecture, where we have a control plane for a centralized UI and API, and data plane(s) where Postgres stacks are hosted.

When deploying a Postgres cluster, we deploy one of the available "Tembo Stacks". Tembo Stacks are Postgres clusters with different combinations of extensions, configurations, metrics, and hardware.

## Components

- **Tembo operator:** the operator is responsible for managing Stacks. The operator depends on [Cloud Native PG](https://cloudnative-pg.io/), and adds capabilities related to Postgres extensions, configuration tuning, and monitoring.
- **Dataplane API:** the API is for serving metrics.
- **Conductor:** this workload receives events from the control plane to make changes in the data plane (not used in self-hosted deployments).

## Security reporting

Please email security issues to security@tembo.io

## License

Tembo Stacks are made available under the [PostgreSQL license](./LICENSE). 
