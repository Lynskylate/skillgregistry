FROM rust:1.93.0-bookworm AS chef

# Install protoc
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app
RUN cargo install cargo-chef

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS cacher
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
COPY . .
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release --bin api --bin worker --bin setup

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl-dev openssl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/api /app/api
COPY --from=builder /app/target/release/worker /app/worker
COPY --from=builder /app/target/release/setup /app/setup

# Default command (can be overridden)
CMD ["/app/api"]
