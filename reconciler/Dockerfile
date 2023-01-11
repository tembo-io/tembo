FROM rust:1.66.0
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release
