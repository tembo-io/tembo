# This does not build on ARM laptops, even with the --platform flag.

FROM --platform=linux/amd64 quay.io/coredb/muslrust:1.68.2-stable AS builder

WORKDIR /build

COPY . .

RUN cargo build --release --target=x86_64-unknown-linux-musl

FROM --platform=linux/amd64 quay.io/coredb/alpine:3.17.3

RUN adduser -D nonroot

COPY --chown=nonroot:nonroot --from=builder /build/target/x86_64-unknown-linux-musl/release/controller /app/controller
COPY --chown=nonroot:nonroot --from=builder /build/target/x86_64-unknown-linux-musl/release/crdgen /app/crdgen

USER nonroot

EXPOSE 8080
ENTRYPOINT ["/app/controller"]
