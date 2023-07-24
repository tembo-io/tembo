[![conductor-deploy workflow](https://github.com/tembo-io/data-plane/actions/workflows/deploy.yml/badge.svg?branch=main)](https://github.com/CoreDB-io/data-plane/actions/workflows/deploy.yml)

# Tembo Stacks

Deploy any data service on PostgreSQL with Tembo Stacks and the Tembo Operator for Kubernetes, packaging community and purpose-built extensions to customize PostgreSQL for a variety of use cases. 

Goodbye database sprawl, hello Postgres.

## Current stacks (all alpha)

- OLTP
- OLAP
- Messaging
- Machine Learning

## Try on Tembo Cloud

- Sign up for Tembo Cloud Waitlist at [tembo.io](https://tembo.io)

Tembo Cloud is a managed service where users can deploy Postgres in various forms. We have a control plane / data plane architecture, where we have a control plane for a centralized UI and API, and data plane(s) where Postgres stacks are hosted. Data planes will be deployable in multiple regions and clouds. 

This code repository is our entire "data plane" codebase, and you can use it to self-host Tembo Stacks.

When deploying a Postgres cluster, we deploy one of the available "Stacks". Stacks are Postgres clusters with different combinations of extensions, configurations, metrics, and hardware.

## Components

- **Tembo operator:** the operator is responsible for managing Stacks. The operator depends on [Cloud Native PG](https://cloudnative-pg.io/), and adds capabilities related to Postgres extensions, configuration tuning, and monitoring.
- **Dataplane API:** the API is for serving metrics.
- **Conductor:** this workload receives events from the control plane to make changes in the data plane.

## Security reporting

Please email security issues to security@tembo.io

## License

Tembo stacks are made available under the [PostgreSQL license](./LICENSE). We also started another free and open source project for building and sharing Postgres extensions called [Trunk](https://github.com/tembo-io/trunk), which is also under the [PostgreSQL license](https://github.com/tembo-io/trunk/blob/main/LICENSE).
