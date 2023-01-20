FROM --platform=linux/amd64 clux/muslrust:stable AS builder

WORKDIR /build

COPY . .

RUN cargo build --release --target=x86_64-unknown-linux-musl

FROM --platform=linux/amd64 alpine:latest

RUN adduser -D nonroot

COPY --chown=nonroot:nonroot --from=builder /build/target/x86_64-unknown-linux-musl/release/controller /app/controller
COPY --chown=nonroot:nonroot --from=builder /build/target/x86_64-unknown-linux-musl/release/crdgen /app/crdgen

USER nonroot

EXPOSE 8080
ENTRYPOINT ["/app/controller"]
