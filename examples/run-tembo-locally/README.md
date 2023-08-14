# Running Tembo locally

This guide is for running a PostgreSQL container locally that supports installing extensions with Trunk. This guide uses Docker Compose to start the same image used in Tembo Cloud on your local machine, and provides guidance on how to manually install and enable extensions. We are also working on a Tembo CLI that will replace this workflow.

## Starting PostgreSQL and connect

- Checkout this directory using git, or duplicate the files in a local directory
- Start a PostgreSQL container locally like this
```
docker-compose up --build -d
```
- The above command will fail if you already have something running on port 5432
- Now, you can connect to your database with this command:
```
psql postgres://postgres:postgres@localhost:5432
```

## Install extensions with Trunk

- Shell into postgres container
```
docker exec -it local-tembo /bin/bash
```

- Trunk install something
```
trunk install pgmq
```

## Enabling extensions

- Connect to postgres, this works from inside or outside the container.
```
psql postgres://postgres:postgres@localhost:5432
```
- Enable an extension
```
CREATE EXTENSION pgmq CASCADE;
```
- List enabled extensions
```
\dx
```
