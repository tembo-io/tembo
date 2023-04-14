FROM quay.io/coredb/c-builder:a1c45b1
ENV PG_VERSION=15
# When Rust 1.68.0 is released, use this instead of nightly
# for enabling sparse registry, a feature required for
# multi-stage build to work.
#
# ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
ENV PATH="/var/lib/postgresql/.cargo/bin:${PATH}"

# Install tcdi/pgx
ARG PGX_VERSION=0.7.1
RUN cargo --version && cargo install -Z sparse-registry cargo-pgx --version "${PGX_VERSION}"
RUN cargo pgx init --pg${PG_VERSION} /usr/bin/pg_config
