LLM_SERVICE_HOST_PORT=http://localhost:8000
RUST_LOG=debug

run:
	LLM_SERVICE_HOST_PORT=${LLM_SERVICE_HOST_PORT} RUST_LOG=${RUST_LOG} cargo run

run-migrations:
	sqlx migrate run

test:
	cargo test -- --ignored