# syntax=docker/dockerfile:1
# Multi-stage build for kaya-cloud (BSL 1.1)

FROM rust:1.87-slim AS builder

WORKDIR /app

# Install system deps
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies: copy only manifests first
COPY apps/backend/Cargo.toml apps/backend/Cargo.lock ./
COPY apps/backend/crates ./crates
COPY apps/backend/bin ./bin
COPY apps/backend/config ./config

# Build release binary
RUN cargo build --release --bin kaya-cloud

# ── Runtime image ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/kaya-cloud ./kaya-cloud
COPY --from=builder /app/config ./config

EXPOSE 8080

CMD ["./kaya-cloud"]
