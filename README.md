# Tembo

[![](https://shields.io/endpoint?url=https://ossrank.com/shield/2103)](https://ossrank.com/p/2103)
[![Discord Chat](https://img.shields.io/discord/1060568981725003789?label=Discord)][Discord]

Tembo aims to improve the experience developers have with deploying, managing, and scaling
Postgres.

Tembo is under active development that you are free to use, except for some parts which may be
licensed to prevent you from competing with the managed service that we are building concurrently
(see [https://tembo.io/tembo-community-license](https://tembo.io/tembo-community-license)).

## Why Postgres?

Postgres is the best OSS database in the world, with millions of active deployments, and is growing faster
than MySQL. It is battle-tested with a large community that can handle SQL (relational) and JSON
(non-relational) queries and a widerange of workloads (i.e, analytical, time-series, geospatial, etc),
on account of its’ rich ecosystem of add-ons and extensions.

## Roadmap

We just got started, but here's what we're working on:

* [Trunk CLI](https://github.com/tembo-io/trunk/tree/main/cli) that users can use to publish and install Postgres extensions
* [Trunk Registry](https://github.com/tembo-io/trunk/tree/main/registry) that serves as a backend for [pgtrunk.io](https://pgtrunk.io), and also provides discovery and metrics
* [pgmq](https://github.com/tembo-io/tembo/tree/main/pgmq) - an easy message queue built with Rust, that we use in our managed service
* A managed service Postgres, which you can get early access to by signing up for
  [our mailing list](https://tembo.io)

In the future:

* A [new UI](https://github.com/tembo-io/tembo/tree/main/pgUI) for Postgres and its extensions
* A [CLI](https://github.com/tembo-io/tembo/tree/main/coredb-cli) built with Rust

## Tembo Cloud

The team at Tembo is building Tembo Cloud, a dev-first, fully-extensible, fully-managed, secure, and
scalable Postgres service. The managed service will provide a growing ecosystem of easily-installed
extensions, allowing you to expand the capabilities of your database.

The service is in private beta, but you can join the waitlist at [https://tembo.io](https://tembo.io).


[Discord]: https://discord.gg/7bGYA9NPux
