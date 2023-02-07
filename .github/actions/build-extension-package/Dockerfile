ARG UBUNTU_VERSION=22.04
FROM ubuntu:${UBUNTU_VERSION}

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=Etc/UTC

# Apt-fast
RUN apt-get update && apt-get install software-properties-common -y

RUN add-apt-repository ppa:apt-fast/stable --yes
RUN apt-get update && apt-get -y install apt-fast

RUN apt-fast install apt-fast curl unzip gpg wget -y

# PostgreSQL
RUN curl https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor | tee /etc/apt/trusted.gpg.d/apt.postgresql.org.gpg > /dev/null
RUN echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list

RUN apt-get update && apt-fast install -y postgresql-${PGVERSION} postgresql-server-dev-${PGVERSION} pkg-config libssl-dev build-essential libclang-dev clang

USER postgres

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/var/lib/postgresql/.cargo/bin:${PATH}"

ARG PGVERSION=15
ARG PGX_VERSION=0.7.1

# Install tcdi/pgx
RUN cargo install cargo-pgx --version $PGX_VERSION
RUN cargo pgx init --pg${PGVERSION} /usr/bin/pg_config

COPY --chown=postgres:postgres . /app
WORKDIR /app

RUN cargo pgx package

# Deb file goes here, mount it on your local system
VOLUME /output
USER root

ARG PACKAGE_VERSION=0.0.0
ARG PACKAGE_NAME=0.0.0

ENV PACKAGE_VERSION=$PACKAGE_VERSION
ENV PACKAGE_NAME=$PACKAGE_NAME
ENV OUTPUT_DIR=/output

# Run
ENTRYPOINT ["bash", "docker/build_ubuntu_deb.sh", "/output"]
