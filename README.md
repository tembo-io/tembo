# CoreDB

[![](https://shields.io/endpoint?url=https://ossrank.com/shield/2103)](https://ossrank.com/p/2103)
[![Discord Chat](https://img.shields.io/discord/1060568981725003789?label=Discord)][Discord]

CoreDB aims to dramatically improve the developer experience of deploying, managing, and scaling
Postgres. We are building a modern, rich Postgres platform with great dev experience,
observability, and extendability.

CoreDB is an "For Everything" distribution of Postgres under active development that you are free to use,
except to compete with the managed service that we are building concurrently with this open project
(see http://coredb.io/coredb-community-license).

## Why Postgres?

Postgres is the best OSS database in the world, with millions of active deployments, and growing faster 
than MySQL. It is battle-tested with a large community that can handle SQL (relational) and JSON 
(non-relational) queries and a widerange of workloads (i.e, analytical, time-series, geospatial, etc), 
on account of its’ rich ecosystem of add-ons and extensions.

## Roadmap

We just got started, but here's what we're working on:

* A [new UI](https://github.com/CoreDB-io/coredb/tree/main/pgUI) for Postgres and its extensions
* A [Kubernetes Operator](https://github.com/CoreDB-io/coredb/tree/main/coredb-operator) built with Rust
* A [CLI](https://github.com/CoreDB-io/coredb/tree/main/coredb-cli) built with Rust
* pgmq - an easy message queue built with Rust, that we use in our managed service, which is available as 
  [a crate](https://github.com/CoreDB-io/coredb/tree/main/crates/pgmq) or as 
  [a Postgres extension](https://github.com/CoreDB-io/coredb/tree/main/extensions/pgmq)
* A managed service Postgres, which you can get early access to by signing up for 
  [our mailing list](https://coredb.io)

[Discord]: https://discord.gg/HjuMB3JX
