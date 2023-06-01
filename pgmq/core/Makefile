POSTGRES_PASSWORD:=postgres

format:
	cargo sqlx prepare
	cargo +nightly fmt --all
	cargo clippy

update.readme:
	cargo readme \
		--no-title \
		--no-indent-headings \
		> README.md

run.postgres:
	docker run --rm -d --name pgmq-pg -e POSTGRES_PASSWORD=${POSTGRES_PASSWORD} -p 5432:5432 quay.io/coredb/pgmq-pg:d03d768

test: run.postgres
	sleep 2;
	echo "Running all tests..."
	cargo test

test.cleanup:
	docker stop postgres

# sets up 
setup.env:
	sqlx migrate run
