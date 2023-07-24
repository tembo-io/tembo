FROM quay.io/coredb/rust:1.66.1 as builder
COPY tembo-operator ./tembo-operator
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release && \
    cargo clean -p conductor
RUN cargo install --path .

FROM quay.io/coredb/debian:11.6-slim
RUN apt-get update && \
    apt-get install -y ca-certificates && apt-get clean && \
    rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
