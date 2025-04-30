FROM quay.io/tembo/rust:1.86-bookworm AS builder
RUN set -eux; \
  apt-get update; \
  apt-get upgrade -y; \
  apt-get autoremove -y; \
  apt-get clean; \
  rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*
WORKDIR /build
COPY . .
RUN cargo build --release

FROM quay.io/tembo/debian:12.10-slim
RUN set -eux; \
  apt-get update; \
  apt-get upgrade -y; \
  apt-get install -y --no-install-recommends openssl ca-certificates; \
  apt-get autoremove -y; \
  apt-get clean; \
  rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*
COPY --from=builder /build/target/release/dataplane_webserver /usr/local/bin/dataplane_webserver
EXPOSE 8080
CMD ["/usr/local/bin/dataplane_webserver"]
