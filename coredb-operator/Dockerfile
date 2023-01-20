FROM clux/muslrust:stable AS builder

COPY . .

RUN cargo build --release

FROM alpine:latest

RUN adduser -D nonroot

COPY --chown=nonroot:nonroot --from=builder target/x86_64-unknown-linux-musl/release/controller /app/controller
COPY --chown=nonroot:nonroot --from=builder target/x86_64-unknown-linux-musl/release/crdgen /app/crdgen

USER nonroot

EXPOSE 8080
ENTRYPOINT ["/app/controller"]
