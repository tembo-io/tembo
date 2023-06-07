FROM rust:1.70-buster as builder
WORKDIR /tembo-pod-init
COPY Cargo.toml .
COPY src/ ./src/
RUN cargo build --release

FROM debian:buster-slim
COPY --from=builder /tembo-pod-init/target/release/tembo-pod-init /usr/bin/tembo-pod-init

CMD ["tembo-pod-init"]
