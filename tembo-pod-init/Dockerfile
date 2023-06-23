FROM rust:1.70-bookworm as builder
WORKDIR /tembo-pod-init
COPY Cargo.toml .
COPY src/ ./src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN set -eux; \
    apt-get update; \
    apt-get upgrade -y; \
    apt-get autoremove -y; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /tembo-pod-init/target/release/tembo-pod-init /usr/bin/tembo-pod-init

CMD ["tembo-pod-init"]
