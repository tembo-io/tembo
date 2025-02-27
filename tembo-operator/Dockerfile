# This does not build on ARM laptops, even with the --platform flag.
FROM quay.io/tembo/muslrust:1.85.0-stable AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --target=x86_64-unknown-linux-musl

FROM cgr.dev/chainguard/static
COPY --chown=nonroot:nonroot --from=builder /build/target/x86_64-unknown-linux-musl/release/controller /app/controller
USER nonroot
EXPOSE 8080
ENTRYPOINT ["/app/controller"]
