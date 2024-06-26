DATABASE_URL:=postgresql://postgres:postgres@localhost:5432/postgres
LLM_SERVICE_HOST_PORT=http://localhost:8000
RUST_LOG=debug
SQLX_OFFLINE:=true

fmt:
	cargo sqlx prepare --database-url ${DATABASE_URL}
	cargo fmt --all
	cargo clippy

check:
	cargo fmt --all --check
	cargo clippy
	cargo sqlx prepare --check

run:
	LLM_SERVICE_HOST_PORT=${LLM_SERVICE_HOST_PORT} RUST_LOG=${RUST_LOG} cargo run

run-migrations:
	sqlx migrate run --database-url ${DATABASE_URL}

run-postgres:
	docker compose up -d postgres

run-mock-server:
	docker compose up -d mock-server

unit-test:
	cargo test

integration-test: run-mock-server
	RUST_LOG=${RUST_LOG} LLM_SERVICE_HOST_PORT=${LLM_SERVICE_HOST_PORT} cargo test ${TEST_NAME} -- --ignored --nocapture

test-all: unit-test integration-test