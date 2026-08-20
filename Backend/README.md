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

## Dependency auditing

[`cargo-deny`](https://embarkstudios.github.io/cargo-deny/) enforces four policies over the
dependency graph: known vulnerabilities (`advisories`), license policy (`licenses`),
disallowed or duplicated crates (`bans`), and where code is allowed to come from
(`sources`). The policy — and the reasoning behind each setting — lives in
[`deny.toml`](deny.toml).

CI runs it on every PR touching `Backend/`, on every push to `main`, and on a weekly cron
(Mondays, 06:00 UTC) via [`backend-audit.yml`](../.github/workflows/backend-audit.yml). The
scheduled run is the one that matters most: an advisory is published against a dependency
you already have, not by a commit, so a vulnerability disclosed on a Tuesday would otherwise
sit undetected until someone next opens a PR. Each check reports as its own job
(`cargo-deny (advisories)`, `cargo-deny (bans licenses sources)`), so a red build says which
policy failed before you open the log.

Run it locally the same way CI does:

```sh
cargo install --locked cargo-deny   # once
cd Backend
cargo deny check
```

### Triaging a flagged advisory

An advisory failure is not automatically a scramble — most are in code paths a given project
never reaches. Work through it in this order.

1. **Read the advisory.** cargo-deny prints the `RUSTSEC-…` id, the affected versions, and a
   link. Note whether a patched version exists.
2. **Find out how it reaches us.** `cargo tree --invert --package <crate>` shows which of our
   dependencies pulls it in, which usually decides whether we can act directly or are waiting
   on an upstream release.
3. **Judge exposure.** Does the vulnerable function sit on a path we actually call, and can
   untrusted input reach it? A parser flaw in a code path handling patient records is a very
   different thing from one in a build-time dependency.
4. **Fix it, in this order of preference:**
   - **Upgrade.** `cargo update --package <crate>` if a patched version exists. Almost always
     the right answer, and the only one that removes the risk rather than accepting it.
   - **Replace or drop** the dependency, if it is unmaintained and no fix is coming.
   - **Accept it explicitly**, only when neither of the above is possible yet.

### Recording an exception

Accepting an advisory means writing it down where the next person will see it — add it to
`ignore` in `deny.toml` with a reason:

```toml
[advisories]
ignore = [
    { id = "RUSTSEC-2024-0000", reason = "Only reachable via the crate's `blocking` feature, which we do not enable. Upstream fix tracked in <issue link>; revisit by 2026-Q4." },
]
```

A good reason states why it does not affect us, and what would make the exception
unnecessary. Exceptions are meant to be temporary: `unused-ignored-advisory = "deny"` makes
CI fail once an entry no longer matches anything, so exceptions get removed when the
dependency is finally upgraded rather than quietly outliving their reason.

The same applies to the other checks — a license outside the allow-list goes in
`[licenses] exceptions`, a crate we deliberately tolerate goes in `[bans] skip` — each with a
comment explaining the call. Nothing should be silently added to the allow-lists to make a
build green.

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
