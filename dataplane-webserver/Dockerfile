FROM quay.io/tembo/rust:1.85-bookworm as builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM quay.io/tembo/rust:1.85.0-chainguard
COPY --from=builder /build/target/release/dataplane_webserver /usr/bin/dataplane_webserver
USER nonroot
EXPOSE 8443

CMD ["/usr/bin/dataplane_webserver"]
