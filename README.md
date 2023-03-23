# CoreDB

[![](https://shields.io/endpoint?url=https://ossrank.com/shield/2103)](https://ossrank.com/p/2103)
[![Discord Chat](https://img.shields.io/discord/1060568981725003789?label=Discord)][Discord]

CoreDB aims to improve the experience developers have with deploying, managing, and scaling
Postgres. 

CoreDB is under active development that you are free to use, except for some parts which may be 
licensed to prevent you from competing with the managed service that we are building concurrently 
(see http://coredb.io/coredb-community-license).

## Why Postgres?

Postgres is the best OSS database in the world, with millions of active deployments, and is growing faster 
than MySQL. It is battle-tested with a large community that can handle SQL (relational) and JSON 
(non-relational) queries and a widerange of workloads (i.e, analytical, time-series, geospatial, etc), 
on account of its’ rich ecosystem of add-ons and extensions.

## Roadmap

We just got started, but here's what we're working on:

* [Trunk CLI](https://github.com/CoreDB-io/coredb/tree/main/trunk/cli) that users can use to publish and install Postgres extensions
* [pgtrunk.io](https://github.com/CoreDB-io/coredb/tree/main/trunk/registry) that serves as a backend for Trunk, and also provides discovery and metrics
* A [Kubernetes Operator](https://github.com/CoreDB-io/coredb/tree/main/coredb-operator) built with Rust
* pgmq - an easy message queue built with Rust, that we use in our managed service, which is available as 
  [a crate](https://github.com/CoreDB-io/coredb/tree/main/crates/pgmq) or as 
  [a Postgres extension](https://github.com/CoreDB-io/coredb/tree/main/extensions/pgmq)
* A managed service Postgres, which you can get early access to by signing up for 
  [our mailing list](https://coredb.io)

In the future:

* A [new UI](https://github.com/CoreDB-io/coredb/tree/main/pgUI) for Postgres and its extensions
* A [CLI](https://github.com/CoreDB-io/coredb/tree/main/coredb-cli) built with Rust

[Discord]: https://discord.gg/7bGYA9NPux
