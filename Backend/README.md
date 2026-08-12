# LockA Backend — Workspace

Rust Cargo workspace for the LockA Backend/API service (Stellar Soroban version). This directory is self-contained: all backend source code, tooling, and build configuration live here, separate from the top-level repository docs.

See [issues.md](issues.md) for the full build breakdown and architecture notes.

## Layout

```text
Backend/
├── Cargo.toml            # workspace manifest (members + shared package metadata)
├── rust-toolchain.toml   # pinned Rust toolchain (rustup will auto-install this version)
├── api/                  # HTTP service binary — Axum-based REST API entrypoint
├── worker/               # background binary — Soroban contract event indexer / async jobs
├── config/               # typed configuration layer (env vars + .env, fail-fast)
├── domain/               # core domain types and business logic, no I/O
├── soroban/              # Stellar/Soroban RPC client, transaction building, contract bindings
├── storage/              # PostgreSQL (sqlx) + encrypted object storage integrations
└── telemetry/            # shared tracing/logging subscriber setup
```

### Crate responsibilities

| Crate | Kind | Responsibility |
| --- | --- | --- |
| `api` | bin | Axum HTTP server exposing the REST endpoints for patients, providers, consent, records, devices, and audit history. |
| `worker` | bin | Long-running background process that indexes Soroban contract events into Postgres read models. |
| `config` | lib | Typed `Settings` loaded from env vars / `.env` — see [Configuration & secrets](#configuration--secrets). |
| `domain` | lib | Core domain types, validation, and business rules, independent of any web framework, database, or chain client. |
| `soroban` | lib | Stellar/Soroban RPC client wrapper, unsigned transaction/XDR building, and generated contract client bindings. |
| `storage` | lib | PostgreSQL access (via `sqlx`) and encrypted object storage (S3-compatible/IPFS) for off-chain records. |
| `telemetry` | lib | Shared `tracing` subscriber setup (`telemetry::init`) used by both binaries — see [Logging](#logging). |

`api` and `worker` are expected to depend on `domain`, `soroban`, `storage`, and `telemetry`; the library crates should not depend on `api` or `worker`.

## Prerequisites

- Rust via [rustup](https://rustup.rs) — the pinned toolchain in `rust-toolchain.toml` will be installed automatically on first use.

## Building

```sh
cd Backend
cargo build
```

## Formatting & linting

Style is defined in `rustfmt.toml`; lint thresholds (complexity, arity, MSRV) are defined in
`clippy.toml`. Lints are gated on the command line with `-D warnings`, not via source-level
`#![deny(...)]` attributes, so CI and local checks stay in sync from one place.

```sh
cargo fmt --check
cargo clippy --workspace -- -D warnings
```

## Logging

Both binaries call `telemetry::init()` once at startup to install a global `tracing` subscriber.

- **Level filter**: standard `RUST_LOG` env var (e.g. `RUST_LOG=info,api=debug`), defaults to `info`.
- **Format**: `LOG_FORMAT=json` for structured production logs; unset (or anything else) for
  human-readable local development output.
- Every span opened with `#[tracing::instrument]` automatically gets a `close` event with
  `time.busy` / `time.idle` fields — instrumenting a function is enough to make its duration
  observable, no manual timing code needed. `soroban` and `storage` declare `tracing` as a
  dependency for this reason; real RPC/DB call sites should follow this convention as they land
  (issues #11 and #9).
- The `api` crate assigns a UUID `x-request-id` to every request (via `tower-http`'s
  `request_id` middleware), attaches it to that request's tracing span, and echoes it back on the
  response header — so a single request can be traced end-to-end through the logs.

```sh
# human-readable, local dev
cargo run -p api
# structured JSON, e.g. for production
LOG_FORMAT=json RUST_LOG=info cargo run -p api
```

## Pre-commit hooks

A git pre-commit hook runs the two commands above automatically, scoped to commits that touch
`Backend/` Rust sources or manifests. Set it up once per clone:

```sh
./Backend/scripts/setup-hooks.sh
```

This points git's `core.hooksPath` at `Backend/.githooks`. To bypass it for a single commit (not
recommended), use `git commit --no-verify`.
