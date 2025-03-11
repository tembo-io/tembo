FROM quay.io/tembo/rust:1.85-bookworm as builder

COPY tembo-operator ./tembo-operator
WORKDIR /tembo-pod-init
COPY Cargo.toml .
COPY src/ ./src/
RUN cargo build --release

FROM quay.io/tembo/rust:1.85.0-chainguard
COPY --from=builder /tembo-pod-init/target/release/tembo-pod-init /usr/bin/tembo-pod-init
USER nonroot
EXPOSE 8443

CMD ["/usr/bin/tembo-pod-init"]
