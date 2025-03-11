FROM quay.io/tembo/rust:1.85-bookworm AS builder

WORKDIR /
COPY tembo-operator ./tembo-operator
WORKDIR /tembo-pod-init
COPY Cargo.toml .
COPY src/ ./src/
RUN cargo build --release

FROM quay.io/tembo/debian:12.9-slim
RUN set -eux; \
  apt-get update; \
  apt-get upgrade -y; \
  apt-get autoremove -y; \
  apt-get clean; \
  rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*

COPY --from=builder /tembo-pod-init/target/release/tembo-pod-init /usr/bin/tembo-pod-init

CMD ["tembo-pod-init"]
