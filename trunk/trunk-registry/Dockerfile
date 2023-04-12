FROM quay.io/coredb/rust:1.68.2 as builder
COPY sqlx-data.json Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations/
RUN cargo build --release && \
    cargo clean -p trunk-registry
RUN cargo install --path .

# second stage.
FROM quay.io/coredb/debian:11.6-slim
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
RUN apt-get update
RUN apt-get install -y --no-install-recommends ca-certificates
RUN update-ca-certificates
