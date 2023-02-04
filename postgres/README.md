# Postgres Docker image

Initially in the CoreDB operator, we were using Docker's official Postgres image https://github.com/docker-library/postgres with no modifications.

Now, we are looking into turning off or on extensions using the CoreDB operator. First, we want to address turning off or on extensions from a list of supported extensions that are already installed. Then, we want to address arbitrary, user-provided extensions.

For initial testing, we are installing some arbitrary extensions from this APT repository https://wiki.postgresql.org/wiki/Apt

## Versioning

The version of the Docker image can be configured in the `Cargo.toml` file in this directory
