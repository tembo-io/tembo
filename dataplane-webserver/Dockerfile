FROM quay.io/coredb/rust:1.70.0 as builder

WORKDIR /build

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

COPY . .

RUN cargo build --release

FROM quay.io/coredb/rust:1.70.0-slim-buster

COPY --from=builder /build/target/release/dataplane_webserver /usr/local/bin/dataplane_webserver

CMD ["/usr/local/bin/dataplane_webserver"]
