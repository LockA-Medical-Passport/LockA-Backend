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
└── storage/              # PostgreSQL (sqlx) + encrypted object storage integrations
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

`api` and `worker` are expected to depend on `config`, `domain`, `soroban`, and `storage`; those library crates should not depend on `api` or `worker`.

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

## Configuration & secrets

Both binaries call `config::Settings::load()` once at startup: it loads `Backend/.env` if one is
present (ignored if missing), then deserializes the process environment into a typed `Settings`
struct. If a required variable is missing, or a value can't be parsed into its expected type, the
process prints a specific, actionable error (e.g. `missing required environment variable:
DATABASE_URL`) and exits immediately — it never runs with partial configuration.

See [`Backend/.env.example`](.env.example) for the full list of variables, which ones are
required vs. optional-with-defaults, and example values. Copy it to `Backend/.env` and fill in
real values for local development:

```sh
cp Backend/.env.example Backend/.env
```

`Settings`'s `Debug` impl redacts secret fields (`OBJECT_STORAGE_ACCESS_KEY_ID`,
`OBJECT_STORAGE_SECRET_ACCESS_KEY`, `JWT_SIGNING_KEY`, `DATABASE_URL`) as `[REDACTED]`, so
accidentally logging a `Settings` value can't leak credentials.

### How secrets are supplied per environment

| Environment | How secrets get in | Committed to source control? |
| --- | --- | --- |
| Local development | `Backend/.env` (copied from `.env.example`, filled in by hand) | Never — gitignored (`Backend/.gitignore`) |
| CI | Repository/organization secrets injected as job env vars by the CI provider | Never |
| Staging / Production | Real environment variables injected by the deployment platform (its own secret manager, e.g. Vault, AWS Secrets Manager, or the platform's built-in env var/secrets store) | Never |

No real secret should ever be committed, including in `.env.example` (which contains only
placeholder values like `changeme`) or in code.

## Pre-commit hooks

A git pre-commit hook runs the two commands above automatically, scoped to commits that touch
`Backend/` Rust sources or manifests. Set it up once per clone:

```sh
./Backend/scripts/setup-hooks.sh
```

This points git's `core.hooksPath` at `Backend/.githooks`. To bypass it for a single commit (not
recommended), use `git commit --no-verify`.
