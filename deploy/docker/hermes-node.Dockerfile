FROM rust:1.88-bookworm AS builder
WORKDIR /workspace

COPY Cargo.toml rust-toolchain.toml ./
COPY crates ./crates

RUN cargo build --release -p hermes-node

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /workspace/target/release/hermes-node /usr/local/bin/hermes-node
COPY configs /app/configs
COPY fixtures /app/fixtures

ENTRYPOINT ["/usr/local/bin/hermes-node"]
