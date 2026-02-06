ARG RUST_VERSION=1.93.0-bookworm
FROM rust:${RUST_VERSION} AS chef

# Configure Cargo mirror.
ARG CARGO_REGISTRIES_CRATES_IO_INDEX=https://github.com/rust-lang/crates.io-index
ENV CARGO_REGISTRIES_CRATES_IO_INDEX=$CARGO_REGISTRIES_CRATES_IO_INDEX
# Also configure sparse index (optional but recommended for speed).
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Install build dependencies once and reuse across build stages.
RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler libprotobuf-dev pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY . .

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,id=skillregistry-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=skillregistry-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=skillregistry-target,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN --mount=type=cache,id=skillregistry-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=skillregistry-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=skillregistry-target,target=/app/target \
    cargo build --release --locked --bin api --bin worker --bin setup \
    && install -Dm755 /app/target/release/api /app/bin/api \
    && install -Dm755 /app/target/release/worker /app/bin/worker \
    && install -Dm755 /app/target/release/setup /app/bin/setup

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 openssl git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/bin/api /app/api
COPY --from=builder /app/bin/worker /app/worker
COPY --from=builder /app/bin/setup /app/setup

# Default command (can be overridden).
CMD ["/app/api"]
