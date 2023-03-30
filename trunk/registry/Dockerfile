FROM rust:1.68.0 as builder
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY registry-s3 ./registry-s3
RUN cargo build && \
    cargo clean -p trunk-registry
RUN cargo install --path .

# second stage.
FROM debian
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
