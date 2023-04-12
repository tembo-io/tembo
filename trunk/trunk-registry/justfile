DATABASE_URL := "postgresql://postgres:postgres@0.0.0.0:5432/postgres"

format:
    cargo +nightly fmt --all
    DATABASE_URL=${DATABASE_URL} cargo clippy
    cargo sqlx prepare

run-migrations:
    sqlx migrate run

run:
    RUST_LOG=debug \
    DATABASE_URL=${DATABASE_URL} \
    RUST_BACKTRACE=full \
    RUST_LOG=debug \
    cargo run
