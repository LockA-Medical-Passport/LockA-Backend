# syntax=docker/dockerfile:1

# Multi-stage build for the `api` service.
#
# Stage 1 compiles the workspace with the full Rust toolchain; stage 2 copies
# out just the release binary, so the shipped image carries no compiler,
# source, or build cache.
#
# Build from the `Backend/` directory (that's the context `docker-compose.yml`
# passes):
#
#     docker build -t locka-api Backend/

# ---- Stage 1: build ---------------------------------------------------------

# Pinned to the same version as `rust-toolchain.toml`. Bump both together.
FROM rust:1.93.1-slim-bookworm AS builder

# Use the toolchain baked into the image instead of resolving
# `rust-toolchain.toml`, which would make rustup fetch `rustfmt` and `clippy`
# on the first build. Those are dev-only components a release build never
# invokes — the compiler version is identical either way.
ENV RUSTUP_TOOLCHAIN=1.93.1

WORKDIR /app

COPY . .

# Cache mounts keep the downloaded crate registry and the `target/` directory
# between builds, so an incremental rebuild only recompiles what changed.
# Neither mount survives into the layer, so the binary is copied to a normal
# path inside the same `RUN` — after this command it would be unreachable.
#
# `--locked` makes the build fail rather than silently resolve different
# dependency versions than `Cargo.lock` records.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --release --locked --package api \
    && cp target/release/api /usr/local/bin/api

# ---- Stage 2: runtime -------------------------------------------------------

FROM debian:bookworm-slim AS runtime

# `ca-certificates` lets the service reach HTTPS endpoints (e.g. pointing
# SOROBAN_RPC_URL at testnet instead of the local network); `curl` backs the
# container healthcheck below.
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged user rather than root.
RUN useradd --create-home --user-group --uid 10001 locka

COPY --from=builder /usr/local/bin/api /usr/local/bin/api

USER locka
# A directory the service can read/write, and deliberately *not* one holding a
# `.env` file: `config::Settings::load()` reads `.env` from the working
# directory when present, and in a container the environment is the only
# configuration source we want.
WORKDIR /home/locka

# Matches the API_BIND_ADDR default in `config`. Documentation only — compose
# publishes the port.
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 \
    CMD curl --fail --silent http://localhost:8080/healthz || exit 1

CMD ["api"]
