# This does not build on ARM laptops, even with the --platform flag.

FROM --platform=linux/amd64 quay.io/tembo/muslrust:1.77.0-stable AS builder

WORKDIR /build

COPY . .

RUN cargo build --release --target=x86_64-unknown-linux-musl

FROM --platform=linux/amd64 quay.io/tembo/alpine:3.18.2

RUN adduser -D nonroot

COPY --chown=nonroot:nonroot --from=builder /build/target/x86_64-unknown-linux-musl/release/controller /app/controller
COPY --chown=nonroot:nonroot --from=builder /build/target/x86_64-unknown-linux-musl/release/crdgen /app/crdgen

USER nonroot

EXPOSE 8080
ENTRYPOINT ["/app/controller"]
