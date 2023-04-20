# Developing a Postgres Extension in Rust using [PGX](https://github.com/tcdi/pgx)

## Start

- Install [Rust](https://rustup.rs/)

- Install [pgx](https://github.com/tcdi/pgx#getting-started)

## Init a new `pgx` project
- https://github.com/tcdi/pgrx/tree/master#getting-started

`cargo pgx run pg15`

- hell world

- use the SPI to execute some sql

- build a background worker

```bash
echo "shared_preload_libraries = 'my_extension.so'" >> ~/.pgx/data-15/postgresql.conf
```

`cargo pgx run pg15`

- tip: watch logs from ~.pgx/
