# CoreDB

CoreDB is being built by a group of long-time Postgres users. We are building a "Postgres on Steroids" distribution, and a managed service to easily provision CoreDB in your cloud or ours.

## Features

* Gitops — all changes to the cluster, database, and schemas should be captured in source control
* Beautiful monitoring UI - it'd be nice to get information about the cluster without having to use the SQL console
* Integrated extensions — developers shouldn't have to enable functionality via cryptic commands
* Awesome migrations — migrations are a database concern, not an application concern. 
* Schema source-of-truth — the database should broadcast to authorized users
* CoreDB Kubernetes Operator


## Maybe also/eventually...

* Serverless - seperate the storage from the compute
* HTAP - automatically store data in the best format (row-wise or columnar) for the queries that you run
* Multi-master — we should be able to run multiple masters and they can sync in the background
* App sidecars - standard directories like `/dbt` `/python` `/rust` that you could drop your data pipelines or apps into, and they run "very close" to the database
