DATABASE_URL:=postgresql://postgres:postgres@localhost:5432/postgres
LLM_SERVICE_HOST_PORT=http://localhost:8000
RUST_LOG=debug
SQLX_OFFLINE:=true

fmt:
	cargo sqlx prepare --database-url ${DATABASE_URL}
	cargo +nightly fmt --all
	cargo clippy

run:
	LLM_SERVICE_HOST_PORT=${LLM_SERVICE_HOST_PORT} RUST_LOG=${RUST_LOG} cargo run

run-migrations:
	sqlx migrate run --database-url ${DATABASE_URL}

run-postgres:
	docker compose up -d postgres

test:
	cargo test -- --ignored