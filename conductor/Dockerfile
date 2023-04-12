FROM quay.io/coredb/rust:1.66.1 as builder
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build && \
    cargo clean -p conductor
RUN cargo install --path .

FROM quay.io/coredb/debian:11.6-slim
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
