FROM rust:1.79.0-slim-buster as builder

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && apt-get clean && \
    rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*

WORKDIR /build

COPY Cargo.toml Cargo.lock /build/
COPY .sqlx/ /build/.sqlx/
COPY src/ /build/src/
COPY migrations/ /build/migrations

RUN SQLX_OFFLINE=true cargo build --release

FROM rust:1.79.0-slim-buster

COPY --from=builder /build/target/release/gateway-daemon /usr/local/bin/gateway-daemon
COPY --from=builder /build/target/release/gateway /usr/local/bin/gateway

CMD ["gateway"]