# WIP: Developing a Postgres Extension in Rust using [PGX](https://github.com/tcdi/pgx)


## Start

- Install [Rust](https://rustup.rs/)

- Install [pgx](https://github.com/tcdi/pgx#getting-started)



## Init a new `pgx` project

```bash
cargo pgx new my_extension
cd my_extension
```

inspect src/lib.rs

- add another function

`cargo pgx run`

- call the functions

- use the SPI to execute some sql

- build a background worker

- tip: watch logs from ~.pgx/