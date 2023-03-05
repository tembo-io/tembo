ARG UBUNTU_VERSION=22.04
FROM ubuntu:${UBUNTU_VERSION}

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=Etc/UTC
ARG PGVERSION=15

RUN apt-get update && \
    apt-get install -y \
      curl unzip gpg wget software-properties-common \
    && curl https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor | tee /etc/apt/trusted.gpg.d/apt.postgresql.org.gpg > /dev/null \
    && echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list \
    && apt-get update  \
    && apt-get install -y \
      postgresql-${PGVERSION} postgresql-server-dev-${PGVERSION} \
      pkg-config libssl-dev build-essential libclang-dev clang && \
    rm -rf /var/lib/apt/lists/*

USER postgres

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/var/lib/postgresql/.cargo/bin:${PATH}"

# Install tcdi/pgx
ARG PGX_VERSION=0.7.1
RUN cargo install cargo-pgx --version "${PGX_VERSION}"
RUN cargo pgx init --pg${PGVERSION} /usr/bin/pg_config
