# CoreDB

[![](https://shields.io/endpoint?url=https://ossrank.com/shield/2103)](https://ossrank.com/p/2103)
[![Discord Chat](https://img.shields.io/discord/1060568981725003789?label=Discord)][Discord]

CoreDB aims to dramatically simplify the developer experience of deploying, managing, and scaling 
Postgres through a novel “database-as-code” approach.

CoreDB is an "On Steroids" distribution of Postgres under active development that you are free to use,
except to compete with the managed service that we are building concurrently with this open project
(see http://coredb.io/coredb-community-license).

The key idea behind CoreDB is to build a radically simplified Postgres platform designed for the 
application developer and built to be extensible—removing the friction from the database management 
workflow by putting the developer first. Developers will be able to deploy a workload-optimized 
Postgres cluster in minutes, with access to all the powerful functionality managed in code, with 
the ability to leverage the rich Postgres ecosystem of plugins, extensions, and support.


## Why Postgres?

Postgres is the second most popular database in the world, behind MySQL. It is a battle-tested database 
with a large community that can handle SQL (relational) and JSON (non-relational) queries and a wide 
range of workloads (i.e, analytical, time-series, geospatial, etc), on account of its’ rich ecosystem 
of add-ons and extensions.

## Deploying

Deploying and managing a Postgres database is complex. There are numerous configurations and optimization 
options. Backup and disaster recovery are non-trivial. Significant resources are required to deploy in 
distributed and highly available modes. These deployments are primarily managed by DBAs (database 
administrators) and data engineers, who spend most of their time optimizing queries, indexes, table 
structures, building new data models, and optimizing the underlying hardware. Application developers 
in larger teams rely on DBAs and data engineers to provision, configure, and manage their Postgres 
databases for their applications.

## Schema Management

In addition to the operational complexity of running the database infrastructure, there is a lack of 
good development interfaces to manage the lifecycle of the data model itself. As a simple example, 
what should happen if you want to change the type of a field in the database? Application developers 
currently have to rely on custom libraries/tooling/process to solve this problem.

## Features

* None yet :)

## Roadmap - MVP

* Gitops — changes to the cluster, database, and schemas are captured in source control
* Beautiful monitoring UI — get information about the cluster, without having to use the SQL console
* Integrated extensions — developers don't have to enable functionality via SQL commands
* Kubernetes Operator

## Roadmap - Next

* Awesome migrations — migrations are a database concern, not an application concern
* Schema source-of-truth — the database should communicate it's schema clearly to authorized users

## Roadmap - Maybe

* Serverless - seperate the storage from the compute
* HTAP - automatically store data in the best format (row-wise or columnar) for the queries that you run
* Multi-master — we should be able to run multiple masters that sync in the background
* App sidecars - standard directories like `/dbt` `/python` `/rust` that you could drop your data pipelines or apps into, and are then run "as close as possible" to the database

[Discord]: https://discord.gg/HjuMB3JX
