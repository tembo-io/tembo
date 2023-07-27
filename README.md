# Tembo

Tembo aims to improve the experience developers have with deploying, managing, and scaling
Postgres. Tembo is under active development that you are free to use anything that we have 
open-sourced.

## Why Postgres?

Postgres is the best OSS database in the world, with millions of active deployments, and is growing faster
than MySQL. It is battle-tested with a large community that can handle SQL (relational) and JSON
(non-relational) queries and a widerange of workloads (i.e, analytical, time-series, geospatial, etc),
on account of its’ rich ecosystem of add-ons and extensions.

## Roadmap

We just got started, but here's what we're working on:

* A managed service Postgres, which you can get early access to by visiting
  [our website](https://tembo.io)
* [Tembo-Stacks](https://github.com/tembo-io/tembo-stacks) - pre-configured Postgres Stacks deployable to Kubernetes
* [Tembo CLI](https://github.com/tembo-io/tembo-cli) built with Rust
* [Trunk CLI](https://github.com/tembo-io/trunk/tree/main/cli) that users can use to publish and install Postgres extensions
* [Trunk Website](https://github.com/tembo-io/trunk/tree/main/registry) that serves as a backend for [pgt.dev](https://pgt.dev), and also provides discovery and metrics

Extensions:
* [pgmq](https://github.com/tembo-io/pgmq) - an easy message queue built with Rust and available as a Postgres extension and Rust crate, that we use in our managed service
* [pg_later](https://github.com/tembo-io/pg_later) - a Postgres extension for completely asynchronous query execution

In the future:

* A [Postgres UI](https://github.com/tembo-io/pgUI) for Postgres and its extensions

## Tembo Cloud

The team at Tembo is building Tembo Cloud, a dev-first, fully-extensible, fully-managed, secure, and
scalable Postgres service. The managed service will provide a growing ecosystem of easily-installed
extensions, allowing you to expand the capabilities of your database.

The service is in private beta, but you can join the waitlist at [https://tembo.io](https://tembo.io).
