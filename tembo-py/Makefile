POETRY_VERSION=1.7.1
SOURCE_OBJECTS=tembo_py tests

format:
	poetry run ruff check --silent --fix --exit-zero ${SOURCE_OBJECTS}
	poetry run black ${SOURCE_OBJECTS}

check:
	poetry run ruff check ${SOURCE_OBJECTS}
	poetry run black --check ${SOURCE_OBJECTS}
	poetry run mypy ${SOURCE_OBJECTS}

setup:
	curl -sSL https://install.python-poetry.org | POETRY_VERSION=${POETRY_VERSION} python3 -
	poetry install

run.postgres:
	docker run -p 5432:5432 --name tembo-postgres -e POSTGRES_PASSWORD=postgres -d quay.io/tembo/vectorize-pg:latest

test: 
	run.postgres
	poetry run pytest -s \
		--ignore=tests/integration_tests \
		--cov=./ \
		--cov-report=xml:coverage-report-unit-tests.xml \
		--junitxml=coverage-junit-unit-tests.xml \
		--cov-report term
