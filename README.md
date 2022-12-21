# CoreDB

[![](https://shields.io/endpoint?url=https://ossrank.com/shield/2103)](https://ossrank.com/p/2103)

CoreDB is an "On Steroids" distribution of Postgres under active development that you are free to use,
except to compete with the managed service that we are building concurrently with this open project
(see http://coredb.io/coredb-community-license).

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
