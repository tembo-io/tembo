SOURCE_OBJECTS=tembo_py tests

format:
	poetry run ruff check --silent --fix --exit-zero ${SOURCE_OBJECTS}
	poetry run black ${SOURCE_OBJECTS}


check:
	poetry run ruff check ${SOURCE_OBJECTS}
	poetry run black --check ${SOURCE_OBJECTS}
	poetry run mypy ${SOURCE_OBJECTS}

test:
	poetry run pytest -s \
		--ignore=tests/integration_tests \
		--cov=./ \
		--cov-report=xml:coverage-report-unit-tests.xml \
		--junitxml=coverage-junit-unit-tests.xml \
		--cov-report term


run.postgres:
	docker run -p 5432:5432 --name tembo-postgres -e POSTGRES_PASSWORD=postgres -d quay.io/tembo/vectorize-pg:latest
