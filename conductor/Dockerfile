FROM quay.io/tembo/rust:1.76-slim-bookworm as builder

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && apt-get clean && \
    rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*

COPY tembo-operator ./tembo-operator
WORKDIR /build

ENV SQLX_OFFLINE=true

COPY sqlx-data.json .
COPY .sqlx/ .sqlx/
COPY Cargo.toml .
COPY Cargo.lock .
COPY ./src ./src
COPY ./migrations ./migrations
COPY metrics.yml .

RUN cargo install --path .

FROM quay.io/tembo/debian:12.5-slim
RUN set -eux; \
    apt-get update; \
    apt-get upgrade -y; \
    apt-get install -y ca-certificates; \
    apt-get clean; \
    apt-get autoremove -y; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
