FROM quay.io/coredb/rust:1.77.0-slim-buster

RUN apt-get update \
    && apt-get install -y pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

COPY . .

RUN cargo build --release

RUN cp /build/target/release/dataplane_webserver /usr/local/bin/dataplane_webserver

CMD ["/usr/local/bin/dataplane_webserver"]
