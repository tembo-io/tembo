# Postgres Docker image

Contains a Dockerfile with a set of standard set of community Postgres extensions, and utilities for managing Postgres with the coredb-operator. Over time, this image will be pruned down to the bare minimum required for the coredb-operator.

## Versioning

The version of the Docker image can be configured in the `Cargo.toml` file in this directory. We may wrap postgres in our CoreDB distribution, but for the time being this crate is just a placeholder to allow for versioning.
