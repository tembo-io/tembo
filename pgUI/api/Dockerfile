FROM quay.io/coredb/rust:1.66.1 as builder
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release && \
    cargo clean -p pgui-api
RUN cargo install --path .

FROM quay.io/coredb/rust:1.66.1-slim-buster
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin/
ENV RUST_LOG=info
