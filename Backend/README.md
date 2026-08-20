# LockA Backend — Workspace

Rust Cargo workspace for the LockA Backend/API service (Stellar Soroban version). This directory is self-contained: all backend source code, tooling, and build configuration live here, separate from the top-level repository docs.

See [issues.md](issues.md) for the full build breakdown and architecture notes.

## Layout

```text
Backend/
├── Cargo.toml            # workspace manifest (members + shared package metadata)
├── rust-toolchain.toml   # pinned Rust toolchain (rustup will auto-install this version)
├── Dockerfile            # multi-stage build of the `api` binary's runtime image
├── docker-compose.yml    # local stack: api + PostgreSQL + local Stellar/Soroban network
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
- [Docker](https://docs.docker.com/get-docker/) with Compose v2, to run the local stack below. Only needed if you want Postgres and a Stellar network without installing them yourself.

## Local stack (Docker Compose)

`docker-compose.yml` runs the whole backend — the API, PostgreSQL, and a self-contained
Stellar/Soroban network — with one command:

```sh
cd Backend
cp .env.example .env
docker compose up
```

That is the entire setup; nothing in `.env` needs editing to get a working local
environment. Once the stack reports healthy:

```sh
curl localhost:8080/healthz   # -> ok
```

| Service | Image | Reachable from the host at | Notes |
| --- | --- | --- | --- |
| `api` | built from `Dockerfile` | <http://localhost:8080> | `GET /healthz` returns `ok` |
| `postgres` | `postgres:16` | `postgres://locka:locka@localhost:5432/locka` | Published so `psql`, migrations, and IDEs can reach the same database the API uses |
| `stellar` | `stellar/quickstart:latest` | <http://localhost:8000> | Everything behind one port: RPC at `/rpc`, Horizon at `/`, friendbot at `/friendbot` |

The first `docker compose up` compiles the workspace in release mode and lets the Stellar
network build its genesis ledger, so give it a few minutes. Later runs start in seconds.
`api` waits for both `postgres` and `stellar` to report healthy before it starts, so a
successful `up` means the whole stack is actually serving, not merely running.

If one of those host ports is already taken — a system-wide PostgreSQL on 5432 is the usual
culprit — set `POSTGRES_PORT`, `STELLAR_PORT`, or `API_PORT` in `.env`. Only the host side
of the mapping moves; the containers keep reaching each other on the standard ports.

The `stellar` service runs the official `stellar/quickstart` image in `--local` mode: a
standalone network that closes a ledger every second, with the fixed network passphrase
`Standalone Network ; February 2017`. Accounts are funded through its bundled friendbot at
<http://localhost:8000/friendbot?addr=YOUR_PUBLIC_KEY>.

Only the `api` binary is containerised. `worker` gets its compose service alongside the
event-indexing work in issue #20.

### How configuration reaches the containers

`.env` supplies the secrets (`JWT_SIGNING_KEY`, the `OBJECT_STORAGE_*` credentials) and any
local overrides you add. The compose file then overrides `DATABASE_URL`, `SOROBAN_RPC_URL`,
and `STELLAR_NETWORK_PASSPHRASE`, because those describe the compose network rather than
your machine — the `localhost` addresses in `.env.example` are for running the binaries
natively and, inside a container, would resolve to the container itself. Compose's
`environment:` takes precedence over `env_file:`, so the container values win without
`.env` having to know about Docker.

### Everyday commands

```sh
docker compose up -d                # start in the background
docker compose ps                   # service status and health
docker compose logs -f api          # follow the API's logs
docker compose up -d --build api    # rebuild and restart after changing Rust code
docker compose down                 # stop, keeping database and ledger state
docker compose down -v              # stop and wipe the volumes (fresh DB, fresh chain)
```

### Faster edit-run loop

The API image bakes in a compiled binary, so every code change needs an image rebuild. For
iterating on the service itself it's quicker to run only its dependencies in Docker and the
API natively:

```sh
docker compose up -d postgres stellar
cargo run -p api
```

For that to talk to the local network rather than testnet, point `.env` at the published
ports:

```sh
SOROBAN_RPC_URL=http://localhost:8000/rpc
STELLAR_NETWORK_PASSPHRASE="Standalone Network ; February 2017"
```

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
