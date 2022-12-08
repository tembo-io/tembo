# CoreDB

CoreDB is an "On Steroids" distribution of Postgres, as well as a managed service to easily provision CoreDB to the cloud.

## Features

* Gitops — all changes to the cluster, database, and schemas are captured in source control
* Beautiful monitoring UI - get information about the cluster, without having to use the SQL console
* Integrated extensions — developers don't have to enable functionality via SQL commands
* Awesome migrations — migrations are a database concern, not an application concern 
* Schema source-of-truth — the database should broadcast to authorized users
* CoreDB Kubernetes Operator

## Maybe also/eventually...

* Serverless - seperate the storage from the compute
* HTAP - automatically store data in the best format (row-wise or columnar) for the queries that you run
* Multi-master — we should be able to run multiple masters and they can sync in the background
* App sidecars - standard directories like `/dbt` `/python` `/rust` that you could drop your data pipelines or apps into, and are then run "as close as possible" to the database
