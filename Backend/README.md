# LockA Backend — Workspace

Rust Cargo workspace for the LockA Backend/API service (Stellar Soroban version). This directory is self-contained: all backend source code, tooling, and build configuration live here, separate from the top-level repository docs.

See [issues.md](issues.md) for the full build breakdown and architecture notes.

## Design documents

| Document | Covers |
| --- | --- |
| [docs/schema.md](docs/schema.md) | PostgreSQL schema for core entities — ERD, table definitions, on-chain vs API provenance per column, and the PII minimization review. Reviewed before migrations are written. |

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
| `domain` | lib | Core domain types, repository traits, and business rules, independent of any web framework, database, or chain client. |
| `soroban` | lib | Stellar/Soroban RPC client wrapper, unsigned transaction/XDR building, and generated contract client bindings. |
| `storage` | lib | PostgreSQL access (via `sqlx`) — the `domain` repository traits' Postgres implementations — and encrypted object storage (S3-compatible/IPFS) for off-chain records. |
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

Before starting the API, generate `JWT_SIGNING_KEY` and `SEP10_SIGNING_SEED` as
described under SEP-10 authentication below. Once the stack reports healthy:

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

`cargo build` needs a reachable, migrated PostgreSQL database (`DATABASE_URL`): `sqlx`'s
`query!`/`query_as!` macros connect to it at compile time to type-check queries against the
real schema. `docker compose up -d postgres` gets you one; see
[Database migrations & connection pooling](#database-migrations--connection-pooling) below.

## Database migrations & connection pooling

Migrations live in [`storage/migrations/`](storage/migrations/) as reversible up/down `.sql`
pairs, applied via [`sqlx`](https://github.com/launchbadge/sqlx). They're a direct
transcription of [`docs/schema.md`](docs/schema.md) — that document is the source of truth for
*why* the schema looks the way it does; a migration diverging from it without a stated reason
is a bug, not a design choice made in passing.

Both `api` and `worker` connect and apply any pending migrations at startup
(`storage::connect`), before doing anything else — the service fails fast and exits if it
can't reach or migrate the database, the same way it already fails fast on invalid
configuration. Calling this from both binaries is safe even if they start concurrently:
`sqlx`'s migrator takes a Postgres advisory lock while applying migrations, so one binary
waits for the other rather than racing it.

Pool size is `DATABASE_MAX_CONNECTIONS` (default 10). `DATABASE_TIMEOUT_SECS`
(default 10) bounds connection acquisition and startup migrations; both must be positive.

### Authoring a migration

```sh
cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features postgres,rustls   # once
cd Backend
sqlx migrate add --source storage/migrations -r <name>       # creates a new <ts>_<name>.up.sql / .down.sql pair
sqlx migrate run --source storage/migrations # apply pending migrations to $DATABASE_URL
sqlx migrate revert --source storage/migrations # roll back the most recent migration
```

### The `.sqlx` offline query cache

`docker build` has no database to check queries against — there's nothing running inside the
build container. To make that work, `Backend/.sqlx/` holds a pre-generated cache of every
`query!`/`query_as!` macro's expected shape, and the `Dockerfile`'s builder stage sets
`SQLX_OFFLINE=true` to use it instead of connecting live. **Whenever a query changes,
regenerate and commit it:**

```sh
cargo sqlx prepare --workspace -- --all-targets
git add .sqlx
```

CI verifies the checked-in cache is current (`cargo sqlx prepare --check`) against the live
database it migrates for every other step, so a forgotten regeneration fails the PR instead of
silently shipping a stale cache in the image.

## Repository layer

Domain/service code never writes SQL directly. `domain` defines a trait per entity group
(`PatientRepository`, `ProviderRepository`, `ProviderStaffRepository`, `ConsentRepository`,
`RecordIndexRepository`, `DeviceRepository`) alongside the plain entity structs and enums they
return; `storage` provides the only implementations (`PgPatientRepository`, etc.), and every
`sqlx` query in the workspace lives there. A repository method never returns `sqlx::Error` —
storage-backend failures are mapped into `domain::RepoError`'s `NotFound` / `Conflict` / `InvalidInput` /
`Backend` variants before crossing that boundary.

Two things worth knowing about the trait shape:

- **Plain `async fn`, not `#[async_trait]`.** Every call site uses a concrete `impl Repository`
  (monomorphized), never `dyn Repository`, so the object-safety `async_fn_in_trait` warns about
  doesn't apply here — silenced deliberately with `#[allow(async_fn_in_trait)]` on each trait.
- **Chain-sourced tables use `upsert_from_chain`, keyed on the table's natural chain id**, and
  are safe under indexer replay: an event carrying a ledger no newer than what's already stored
  is a no-op rather than a regression (`patients`, `provider_staff`, `access_requests`,
  `consent_grants`, `device_registrations`). Tables with a real API write path instead (`providers`,
  `record_index`) split creation (`register`/`insert`) from the specific chain-driven fields
  that update later (`update_verification_status_from_chain`/`mark_anchored`) — see the doc
  comments in `domain/src/*.rs` for the reasoning behind each table's specific approach.

For mutable ledger-versioned rows, the indexer must coalesce changes into one final
snapshot per entity per ledger before calling the repository: equal-ledger writes
are ignored. Ledger numbers alone cannot order multiple events inside one ledger.
Device revocation is terminal and uses a status guard instead of a ledger watermark.

Tests use `#[sqlx::test]`: a fresh, migrated, throwaway database per test, created from
`DATABASE_URL`. Run them the same way as any other test:

```sh
cargo test -p storage -p domain
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

## Soroban RPC

`soroban::SorobanRpc` is the internal async interface. `SorobanRpcClient` wraps the
[official SDF Rust RPC client](https://github.com/stellar/rs-stellar-rpc-client) and
`stellar-xdr` types for network info, account loading, simulation, submission,
transaction lookup, and paginated events. `FakeSorobanRpcClient` scripts responses
or errors per method and records calls without network access. Unscripted calls fail.

Set `SOROBAN_RPC_URL` and `STELLAR_NETWORK_PASSPHRASE` together for testnet or the
local quickstart. Both binaries verify the reported passphrase at startup.
`SOROBAN_RPC_TIMEOUT_SECS` defaults to 15 per attempt; `SOROBAN_RPC_MAX_RETRIES`
defaults to 3 (maximum 5). Network failures, timeouts, HTTP 408/429/5xx use bounded
exponential backoff with jitter. Permanent HTTP and JSON-RPC errors are returned
immediately. Submission preserves statuses such as `ERROR` and `TRY_AGAIN_LATER`
for the caller; a retry sends the identical envelope, never a rebuilt transaction.

The opt-in smoke test loads a friendbot-funded account and simulates an empty-footprint
TTL extension, which changes no application state and is never submitted:

```sh
# Start only the chain; no API secrets needed.
docker compose up -d stellar
# Fund a public test account via http://localhost:8000/friendbot?addr=G...
SOROBAN_RPC_URL=http://localhost:8000/rpc \
STELLAR_NETWORK_PASSPHRASE='Standalone Network ; February 2017' \
SMOKE_ACCOUNT=G... cargo test -p soroban local_network_simulates -- --ignored
```

## SEP-10 authentication

The [SEP-10 protocol](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md)
is implemented for non-custodial `G...` wallets. Muxed accounts, memo-based custodial
identities, and client-domain attestation are explicitly unsupported. Existing
accounts must meet their on-chain medium threshold using distinct Ed25519 signers;
unfunded accounts require their master signature. A failed account lookup never
falls back to unfunded-account verification.

Generate separate server credentials, then put them in `.env` or your secret manager:

```sh
openssl rand -hex 32                           # JWT_SIGNING_KEY
cargo run -p soroban --example generate_auth_seed # SEP10_SIGNING_SEED
```

`SEP10_HOME_DOMAIN` is the application's domain. `SEP10_WEB_AUTH_DOMAIN` is the auth
server hostname and must match `SEP10_WEB_AUTH_ENDPOINT` (the full public URI ending
in `/auth/challenge`). HTTPS is required except on localhost. Changing the externally
published API port also requires updating that endpoint. `AUTH_CHALLENGE_TTL_SECS`
defaults to 900 (maximum 900); `AUTH_TOKEN_TTL_SECS` defaults to 900 (maximum 3600).
Only the dedicated server seed is held by the backend; patients/providers sign locally.

1. `GET /auth/challenge?account=G...` returns `{transaction, network_passphrase}`.
   An optional `home_domain` must match configuration.
2. Sign the returned envelope with Freighter using that network passphrase, preserving
   the server signature. The sequence is zero; do not submit it to Stellar.
3. `POST /auth/verify` with JSON `{"transaction":"SIGNED_XDR"}` returns
   `{token, token_type: "Bearer", expires_in}`. Form encoding is also supported.
   For standard SEP-10 discovery, POST to `/auth/challenge` is an equivalent token endpoint.
4. Send `Authorization: Bearer TOKEN` to protected routes. `GET /auth/me` returns the
   authenticated account. Future protected routers must use `auth::authenticate` and
   read `Extension<AuthenticatedAccount>`; a session authenticates identity, while
   patient/provider permissions remain a separate domain authorization decision.

`/.well-known/stellar.toml` publishes the public signing key, network passphrase,
and auth endpoint. Serve this document at the home domain too if authentication is
hosted on a different domain. Auth endpoints support CORS preflight and disable caching.
All auth errors use `{code, message, error}` with a safe message; database/RPC details,
JWTs, seeds, and signed envelopes are never included in request logs.

Challenges use random 48-byte nonces. Only transaction hashes, account IDs, expiry,
and consumption timestamps are stored. Atomic consumption rejects replays across
concurrent requests, restarts, and multiple replicas. Expired challenges are removed
by the API every minute and by the worker on its tick. JWT verification pins HS256,
issuer, audience, issue time, and expiration. Rotating the JWT key invalidates existing
sessions; rotating the SEP-10 seed invalidates outstanding challenges.

## Testing and coverage

Keep unit tests in each module's `#[cfg(test)] mod tests` (large modules may use
`tests.rs`). Use descriptive behavior names. Inject traits for external dependencies;
use `FakeSorobanRpcClient` for service tests. RPC transport tests use an ephemeral local
HTTP server and exercise the official client's encoding, retries, and error handling.
API tests use the real Axum router with `tower::ServiceExt::oneshot` and real signatures.

Repository tests use `#[sqlx::test]`, creating a fresh migrated Postgres database per
test and cleaning it up afterward. `DATABASE_URL` must name a disposable database and
its role must have `CREATEDB`. These are integration tests against real PostgreSQL,
not SQLite emulations. They cover constraints, replay ordering, filtering, and concurrent
challenge consumption. SQL remains exclusively in `storage`, including test fixtures.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo install cargo-llvm-cov --locked --version 0.6.24
rustup component add llvm-tools-preview
mkdir -p target/coverage
cargo llvm-cov --workspace --all-targets --lcov --output-path target/coverage/lcov.info --fail-under-lines 60
```

CI generates and uploads the LCOV report on every backend PR, with an initial 60%
workspace line-coverage floor (including domain, storage, API, and Soroban code).
Live-chain smoke tests remain opt-in; deterministic protocol and transport tests run
in the standard suite. To build without a database, use `SQLX_OFFLINE=true`; database
tests still require a real server at runtime.
