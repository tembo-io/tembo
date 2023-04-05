FROM rust:1.68.0 as builder
COPY sqlx-data.json Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations/
RUN cargo build --release && \
    cargo clean -p trunk-registry
RUN cargo install --path .

# second stage.
FROM debian
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
