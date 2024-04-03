FROM quay.io/coredb/rust:1.71.0-slim-buster

FROM rust:latest as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

RUN apt-get update \
    && apt-get install -y pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

COPY . .

RUN cargo build --release

RUN cp /build/target/release/dataplane_webserver /usr/local/bin/dataplane_webserver

CMD ["/usr/local/bin/dataplane_webserver"]
