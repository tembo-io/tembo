# How it could work

## Given

* A CoreDB cluster is deployed
* I have user credentials to deploy to it
* I have created a coredb project on my machine via `coredb init` command
* I create files according to coredb docs

## When

* I type `coredb deploy`

## Then

* My files are analyzed and good stuff happens in the CoreDB cluster
  * `db/schemas` directory is parsed for changes, and migrations are generated
    * potentially problematic migrations are flagged
    * the cli steps me through the migrations one-by-one
  * `instances` directory is parsed for instances of postgres to spin up, within which dataplane clusters, and with what limitations (this assumes that we have magic multi-master functionality, which doesn't exist yet)
  * `apps` directory is parsed for apps to bundle into the postgres deployment (initially, only `actix` framework)