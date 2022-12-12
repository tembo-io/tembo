# postgres-rust-api

A proof-of-concept Rust HTTP webserver running inside Postgres.

## Setup instructions

### System Requirements

Install the system requirements for pgx: https://github.com/tcdi/pgx#system-requirements

### PGX Setup

Install the cargo-pgx subcommand and init the dev environment. Note that this may take a few minutes.

```bash
cargo install --locked cargo-pgx
cargo pgx init
```

### Enable the extension

Must set `shared_preload_libraries` in postgresql.conf in order for the background worker to be loaded.

```bash
echo "shared_preload_libraries = 'api.so'" >> ~/.pgx/data-14/postgresql.conf
```

### Running the server

```bash
cargo pgx run pg14
```

## API Server

The API server is implemented in Rust and runs inside Postgres. It is a simple list of titles that can be created, listed, and deleted. It also has an endpoint to echo back the body.

Everything is backed by the `items` table.

### Create a title

```bash
curl -X POST -H "Content-Type: application/json" -d '{"title": "My Title"}' http://0.0.0.0:8080/add
```

### List titles

```bash
curl http://0.0.0.0:8080
```

### Delete a title

```bash
curl http://0.0.0.0:8080/delete/:id
```

### Echo

```bash
curl http://0.0.0.0:8080/echo
```
