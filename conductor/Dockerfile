FROM quay.io/coredb/rust:1.71.0-slim-buster as builder

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && apt-get clean && \
    rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*

COPY tembo-operator ./tembo-operator
WORKDIR /build

COPY Cargo.toml .
COPY Cargo.lock .
COPY ./src ./src

RUN cargo install --path .

FROM quay.io/coredb/debian:11.6-slim
RUN apt-get update && \
    apt-get install -y ca-certificates && apt-get clean && \
    rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
