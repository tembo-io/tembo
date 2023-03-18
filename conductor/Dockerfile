FROM rust:1.66.1 as builder
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build && \
    cargo clean -p conductor
RUN cargo install --path .

# second stage.
FROM debian
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
