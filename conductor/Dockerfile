FROM quay.io/tembo/rust:1.85-bookworm as builder
WORKDIR /
COPY tembo-operator ./tembo-operator
WORKDIR /build

ENV SQLX_OFFLINE=true

COPY sqlx-data.json .
COPY .sqlx/ .sqlx/
COPY Cargo.toml .
COPY Cargo.lock .
COPY ./src ./src
COPY ./migrations ./migrations
COPY metrics.yml .

RUN cargo build --release

FROM quay.io/tembo/rust:1.85.0-chainguard
COPY --from=builder /build/target/release/conductor /usr/bin/conductor
USER nonroot
EXPOSE 8443

CMD ["/usr/bin/conductor"]

