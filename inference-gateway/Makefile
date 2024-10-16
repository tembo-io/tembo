DATABASE_URL:=postgresql://postgres:postgres@localhost:5432/postgres
MODEL_SERVICE_PORT_MAP=facebook/opt-125m=http://localhost:8000
RUST_LOG=debug
SQLX_OFFLINE:=true
ORG_AUTH_ENABLED:=false

fmt:
	cargo sqlx prepare --database-url ${DATABASE_URL}
	cargo fmt --all
	cargo clippy

prepare:
	cargo sqlx prepare

check:
	cargo fmt --all --check
	cargo clippy
	cargo sqlx prepare --check

run:
	MODEL_SERVICE_PORT_MAP=${MODEL_SERVICE_PORT_MAP} RUST_LOG=${RUST_LOG} ORG_AUTH_ENABLED=${ORG_AUTH_ENABLED} cargo run

run-migrations:
	sqlx migrate run --database-url ${DATABASE_URL}

run-postgres:
	docker compose up -d postgres

run-mock-server:
	docker compose up -d mock-server

unit-test:
	cargo test

integration-test: run-mock-server
	RUST_LOG=${RUST_LOG} MODEL_SERVICE_PORT_MAP=${MODEL_SERVICE_PORT_MAP} cargo test ${TEST_NAME} -- --ignored --nocapture --threads 1

test-all: unit-test integration-test