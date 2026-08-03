# LockA Backend — Build Issues

This file tracks the issue breakdown for building the LockA Backend/API service on Stellar using the Soroban SDK, in Rust. All source code for this build lives inside this `Backend/` directory.

Each issue below has been opened on GitHub (see the linked issue number). Checkboxes inside each issue body track sub-tasks; check them off as work lands, and close the corresponding GitHub issue when its acceptance criteria are met.

## Architecture Notes

- **Language & runtime:** Rust, async via `tokio`.
- **Web framework:** Axum (Tower-based), with `utoipa` for OpenAPI generation.
- **Database:** PostgreSQL via `sqlx`, for application metadata/indexes only — never raw medical records.
- **Chain integration:** Stellar/Soroban Rust SDK + Soroban RPC (simulate/submit/get-events). The backend never holds patient/provider private keys — it builds unsigned transaction XDR for the client to sign with **Freighter**, and relays signed envelopes back to the network. Authentication uses the **SEP-10** Web Authentication standard (sign-a-challenge, not password/key custody).
- **Encrypted storage:** S3-compatible/IPFS object storage with envelope encryption; only ciphertext hashes are anchored on-chain via `RecordCommitmentRegistry`.
- **Indexing:** a dedicated worker polls Soroban `get-events` for all LockA contracts and maintains Postgres read models, so the API never queries the chain synchronously on the read path.
- **Testing pyramid:** unit tests (mocked/fake RPC + repositories) → integration tests (`testcontainers` Postgres) → contract-interaction tests (local Soroban network) → end-to-end tests (full docker-compose stack).

## Issue Index


### A. Project Foundation & Tooling

- [ ] **#1** Scaffold Rust Cargo workspace inside Backend/ — [GH #2](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/2)
- [ ] **#2** Configure rustfmt, clippy, and pre-commit checks — [GH #3](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/3)
- [ ] **#3** Set up structured logging with tracing — [GH #4](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/4)
- [ ] **#4** Configuration & secrets management — [GH #5](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/5)
- [ ] **#5** GitHub Actions CI pipeline (build, fmt, clippy, test) — [GH #6](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/6)
- [ ] **#6** Docker & docker-compose local dev environment — [GH #7](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/7)
- [ ] **#7** Dependency security auditing in CI (cargo-audit / cargo-deny) — [GH #8](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/8)

### B. Data Layer

- [ ] **#8** Design PostgreSQL schema for core entities — [GH #9](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/9)
- [ ] **#9** Set up sqlx migrations & connection pooling — [GH #10](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/10)
- [ ] **#10** Implement repository/data-access layer for core entities — [GH #11](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/11)

### C. Stellar & Soroban Integration

- [ ] **#11** Integrate Stellar/Soroban Rust SDK & RPC client wrapper — [GH #12](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/12)
- [ ] **#12** Implement SEP-10 wallet authentication (Freighter challenge/response, JWT issuance) — [GH #13](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/13)
- [ ] **#13** Implement unsigned transaction/XDR builder service for client-side signing — [GH #14](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/14)
- [ ] **#14** Integrate PatientIdentityRegistry contract client — [GH #15](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/15)
- [ ] **#15** Integrate ProviderRegistry contract client — [GH #16](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/16)
- [ ] **#16** Integrate ConsentAccessControl contract client — [GH #17](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/17)
- [ ] **#17** Integrate RecordCommitmentRegistry contract client — [GH #18](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/18)
- [ ] **#18** Integrate DeviceAttestationRegistry contract client — [GH #19](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/19)
- [ ] **#19** Integrate AuditEventEmitter contract client — [GH #20](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/20)
- [ ] **#20** Build Soroban contract event indexer worker — [GH #21](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/21)

### D. Core Domain Services & API

- [ ] **#21** Patient identity API endpoints — [GH #22](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/22)
- [ ] **#22** Provider registry API endpoints — [GH #23](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/23)
- [ ] **#23** Consent & access-request API endpoints — [GH #24](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/24)
- [ ] **#24** Encrypted off-chain record storage service — [GH #25](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/25)
- [ ] **#25** Record upload/retrieval API with on-chain commitment anchoring — [GH #26](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/26)
- [ ] **#26** Device attestation & IoT reading ingestion API — [GH #27](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/27)
- [ ] **#27** Audit log API (patient-facing access history) — [GH #28](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/28)
- [ ] **#28** Notification service (email/SMS/push) — [GH #29](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/29)
- [ ] **#29** Zero-knowledge proof verification service & API — [GH #30](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/30)
- [ ] **#30** AI-assisted patient summary service & API — [GH #31](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/31)

### E. Cross-Cutting Concerns

- [ ] **#31** API input validation & centralized error handling — [GH #32](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/32)
- [ ] **#32** Rate limiting & abuse-protection middleware — [GH #33](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/33)
- [ ] **#33** OpenAPI documentation generation (utoipa) published from code — [GH #34](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/34)
- [ ] **#34** Observability: metrics & distributed tracing (OpenTelemetry) — [GH #35](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/35)

### F. Testing

- [ ] **#35** Unit test suite conventions & coverage reporting — [GH #36](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/36)
- [ ] **#36** Integration test suite with testcontainers (Postgres) — [GH #37](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/37)
- [ ] **#37** Contract-interaction test suite against local Soroban network — [GH #38](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/38)
- [ ] **#38** End-to-end API test suite — [GH #39](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/39)

---

## A. Project Foundation & Tooling


### Issue 1: Scaffold Rust Cargo workspace inside Backend/ — [GH #2](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/2)

**Labels:** backend, infra

**Objective:** Create the initial Rust Cargo workspace that will house the LockA backend/API service, located under the `Backend/` directory of this repository (not the repo root).

**Tasks:**
- [ ] Initialize a Cargo workspace at `Backend/` with a `Cargo.toml` workspace manifest.
- [ ] Create initial member crates, e.g. `Backend/api` (HTTP service binary), `Backend/domain` (core domain types/logic), `Backend/soroban` (Stellar/Soroban integration), `Backend/storage` (DB + object storage), `Backend/worker` (indexer/background jobs).
- [ ] Add a root `Backend/.gitignore` for Rust build artifacts (`target/`, `.env`, etc.).
- [ ] Add a `Backend/rust-toolchain.toml` pinning the Rust version used across the team.
- [ ] Document the workspace layout in `Backend/README.md`.

**Acceptance Criteria:**
- `cargo build` succeeds from within `Backend/` with no member crates yet doing real work.
- Workspace layout and each crate's purpose are documented.


### Issue 2: Configure rustfmt, clippy, and pre-commit checks — [GH #3](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/3)

**Labels:** backend, infra

**Objective:** Establish consistent formatting and linting standards across the workspace, enforced locally and in CI.

**Tasks:**
- [ ] Add `Backend/rustfmt.toml` with agreed style rules.
- [ ] Add a `Backend/clippy.toml` and enable `#![deny(warnings)]`-equivalent lint gating in CI (not necessarily in source).
- [ ] Add a pre-commit hook (e.g. via `lefthook` or a simple git hook script) running `cargo fmt --check` and `cargo clippy -- -D warnings`.
- [ ] Document how contributors install and run the hooks.

**Acceptance Criteria:**
- `cargo fmt --check` and `cargo clippy -- -D warnings` both run cleanly on a fresh scaffold.
- Pre-commit hook is documented and runnable locally.


### Issue 3: Set up structured logging with tracing — [GH #4](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/4)

**Labels:** backend, infra

**Objective:** Provide consistent, structured, leveled logging across all crates using the `tracing` ecosystem, so requests, background jobs, and Soroban interactions are all traceable.

**Tasks:**
- [ ] Add `tracing`, `tracing-subscriber`, and `tracing-appender` (or equivalent) to the workspace.
- [ ] Configure JSON-formatted logs for production and human-readable logs for local development, gated by an env var.
- [ ] Add request-scoped tracing spans (correlation/request IDs) in the API crate.
- [ ] Ensure Soroban RPC calls and DB queries emit spans with timing information.

**Acceptance Criteria:**
- Logs include timestamp, level, target module, and request/correlation ID where applicable.
- Log format is switchable between JSON and pretty-printed via configuration.


### Issue 4: Configuration & secrets management — [GH #5](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/5)

**Labels:** backend, infra, security

**Objective:** Provide a single, typed configuration layer for the service (DB URL, Soroban RPC endpoint, network passphrase, object storage credentials, JWT signing keys, etc.), sourced from environment variables with sane defaults for local dev.

**Tasks:**
- [ ] Add a config crate/module using `config` + `serde` (or `envy`) to load typed settings from environment variables and optional `.env` files.
- [ ] Add `Backend/.env.example` documenting every required variable, with no real secrets.
- [ ] Fail fast at startup with a clear error if required configuration is missing or malformed.
- [ ] Document how secrets are expected to be supplied in each environment (local, CI, staging, production) without committing them to source control.

**Acceptance Criteria:**
- Service refuses to start with a descriptive error when required config is missing.
- `.env.example` covers every configuration key used by the service.


### Issue 5: GitHub Actions CI pipeline (build, fmt, clippy, test) — [GH #6](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/6)

**Labels:** backend, infra

**Objective:** Automate build verification on every push/PR so regressions are caught before merge.

**Tasks:**
- [ ] Add `.github/workflows/backend-ci.yml` triggered on PRs/pushes touching `Backend/**`.
- [ ] Run `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` with a Postgres service container available.
- [ ] Cache Cargo registry and build artifacts to keep CI fast.
- [ ] Fail the workflow on any non-zero exit from the above steps.

**Acceptance Criteria:**
- A PR with a formatting violation or failing test fails CI.
- A clean PR passes CI in a reasonable time (cache hit reduces cold-build time).


### Issue 6: Docker & docker-compose local dev environment — [GH #7](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/7)

**Labels:** backend, infra

**Objective:** Let contributors spin up the full local stack (API, PostgreSQL, and a local Soroban/Stellar network) with one command.

**Tasks:**
- [ ] Add a multi-stage `Backend/Dockerfile` for the API service.
- [ ] Add `Backend/docker-compose.yml` with services for the API, PostgreSQL, and a local Stellar/Soroban network (e.g. the official `stellar/quickstart` image running in `--local` mode).
- [ ] Wire environment variables so the API container points at the compose Postgres and Soroban RPC endpoints by default.
- [ ] Document the `docker compose up` workflow in `Backend/README.md`.

**Acceptance Criteria:**
- `docker compose up` brings up a working API reachable on localhost, backed by Postgres and a local Soroban network.
- No manual configuration steps are required beyond copying `.env.example` to `.env`.


### Issue 7: Dependency security auditing in CI (cargo-audit / cargo-deny) — [GH #8](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/8)

**Labels:** backend, infra, security

**Objective:** Continuously check dependencies for known vulnerabilities and disallowed licenses.

**Tasks:**
- [ ] Add `cargo-deny` (or `cargo-audit`) configuration covering advisories, license policy, and banned crates.
- [ ] Add a CI job running the audit on every PR and on a scheduled weekly cron.
- [ ] Document the process for triaging and addressing flagged advisories.

**Acceptance Criteria:**
- CI fails when a dependency has a known critical/high vulnerability without an explicit, documented exception.
- Weekly scheduled run is configured and visible in Actions history.


---

## B. Data Layer


### Issue 8: Design PostgreSQL schema for core entities — [GH #9](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/9)

**Labels:** backend, data

**Objective:** Design the relational schema for application-side metadata that mirrors and indexes on-chain state: patients, providers, access requests/consent, record index entries, device registrations, and notification records. No raw medical record content is stored in this schema — only encrypted-blob references and metadata.

**Tasks:**
- [ ] Define ER diagram / table list: `patients`, `providers`, `provider_staff`, `access_requests`, `consent_grants`, `record_index`, `device_registrations`, `device_readings_index`, `notifications`, `audit_log_index`.
- [ ] Define foreign keys, indexes, and constraints (e.g. unique Stellar account per patient/provider).
- [ ] Document which fields are populated from on-chain events vs. submitted directly by API requests.
- [ ] Review schema for PII minimization — store only what off-chain indexing genuinely requires.

**Acceptance Criteria:**
- Schema design is documented (ERD or equivalent) and reviewed before migrations are written.
- No table stores raw medical record content, only encrypted-storage references and hashes.


### Issue 9: Set up sqlx migrations & connection pooling — [GH #10](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/10)

**Labels:** backend, data

**Objective:** Implement the schema from #8 as versioned SQL migrations and wire up a pooled async database connection using `sqlx`.

**Tasks:**
- [ ] Add `sqlx` with the `postgres` and `runtime-tokio` features, plus `sqlx-cli` for migration management.
- [ ] Write initial migration(s) under `Backend/storage/migrations/` implementing the schema from #8.
- [ ] Configure a connection pool (`PgPool`) sized from configuration, with health-check on startup.
- [ ] Enable `sqlx` compile-time query checking against a local database in CI.

**Acceptance Criteria:**
- `sqlx migrate run` applies cleanly to a fresh database.
- Service fails fast on startup if it cannot reach or migrate the database.


### Issue 10: Implement repository/data-access layer for core entities — [GH #11](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/11)

**Labels:** backend, data

**Objective:** Provide a typed repository layer (traits + Postgres implementations) over the schema from #8/#9, so domain/service code never writes raw SQL directly.

**Tasks:**
- [ ] Define repository traits per entity (e.g. `PatientRepository`, `ProviderRepository`, `ConsentRepository`, `RecordIndexRepository`, `DeviceRepository`).
- [ ] Implement Postgres-backed versions using `sqlx`.
- [ ] Add unit tests using an in-memory or test-schema Postgres instance (see #36).
- [ ] Ensure repositories return domain error types, not raw `sqlx::Error`.

**Acceptance Criteria:**
- Each repository trait has a working Postgres implementation with passing unit tests.
- No SQL string literals exist outside the storage crate.


---

## C. Stellar & Soroban Integration


### Issue 11: Integrate Stellar/Soroban Rust SDK & RPC client wrapper — [GH #12](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/12)

**Labels:** backend, soroban

**Objective:** Establish the foundational client for talking to Stellar/Soroban: account loading, network passphrase handling, and Soroban RPC (simulate/submit/get-events) wrapped behind an internal, testable interface.

**Tasks:**
- [ ] Add the official Stellar Rust SDK / Soroban client crates to the `soroban` crate.
- [ ] Implement a `SorobanRpcClient` wrapper around simulate-transaction, send-transaction, get-transaction, and get-events RPC calls, with retry/backoff on transient network errors.
- [ ] Support both testnet and local-network (quickstart) configurations via config from #4.
- [ ] Add a mock/fake implementation of the client for use in unit tests that don't need a live network.

**Acceptance Criteria:**
- Wrapper can fetch network info and simulate a no-op transaction against a local Soroban network.
- A fake client implementation exists and is usable in tests without network access.


### Issue 12: Implement SEP-10 wallet authentication (Freighter challenge/response, JWT issuance) — [GH #13](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/13)

**Labels:** backend, soroban, security

**Objective:** Implement Stellar's standard SEP-10 Web Authentication protocol so patients and providers can authenticate by signing a challenge transaction with Freighter, without ever exposing a private key to the backend. On success, issue a short-lived JWT/session token for subsequent API calls.

**Tasks:**
- [ ] Implement `GET /auth/challenge` returning a SEP-10 challenge transaction XDR for a given Stellar public key.
- [ ] Implement `POST /auth/verify` that validates the signed challenge (signature, time bounds, source/domain checks) per the SEP-10 spec.
- [ ] On successful verification, issue a signed JWT (or opaque session token backed by a `sessions` table) tied to the Stellar account.
- [ ] Add middleware to authenticate subsequent requests via the issued token.
- [ ] Write tests covering expired challenges, wrong signer, and replay attempts.

**Acceptance Criteria:**
- A client can complete the challenge/verify flow using a real Freighter-signed transaction against testnet and receive a valid session token.
- Expired, malformed, or wrongly-signed challenges are rejected with clear errors.


### Issue 13: Implement unsigned transaction/XDR builder service for client-side signing — [GH #14](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/14)

**Labels:** backend, soroban

**Objective:** Since the backend never holds patient/provider private keys, it must build unsigned Soroban contract-invocation transactions (as XDR) for the frontend to sign via Freighter and submit back for relaying.

**Tasks:**
- [ ] Implement a generic `build_contract_invocation(contract_id, function, args, source_account)` helper producing simulated, fee-bumped, unsigned transaction XDR.
- [ ] Implement an endpoint/service to accept a client-signed XDR envelope and submit it via the RPC client from #11, polling until final status.
- [ ] Handle simulation failures (e.g. contract errors) by surfacing structured, actionable error responses instead of raw XDR/RPC errors.
- [ ] Add tests using the fake RPC client from #11.

**Acceptance Criteria:**
- A caller can request an unsigned transaction for a given contract call, sign it externally, and submit it through the backend to reach the local Soroban network.
- Contract simulation errors are translated into clear, typed API errors.


### Issue 14: Integrate PatientIdentityRegistry contract client — [GH #15](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/15)

**Labels:** backend, soroban

**Objective:** Provide typed Rust bindings and a service wrapper for calling and reading the `PatientIdentityRegistry` Soroban contract (patient passport identifiers, public keys, recovery configuration, identity commitments).

**Tasks:**
- [ ] Generate or hand-write Rust client bindings for the contract's interface (coordinate with `locka-contracts` repo for the interface/WASM).
- [ ] Implement a `PatientIdentityService` with methods to register a passport, fetch identity state, and update recovery configuration, using the XDR builder from #13 for writes and RPC reads for queries.
- [ ] Sync relevant on-chain state into the `patients` table (see #8) via the indexer (#20) rather than querying the chain on every read.
- [ ] Add contract-interaction tests (see #37) against a local Soroban network with the contract deployed.

**Acceptance Criteria:**
- Backend can register a patient passport on-chain and read back its state from a local Soroban network in a test.


### Issue 15: Integrate ProviderRegistry contract client — [GH #16](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/16)

**Labels:** backend, soroban

**Objective:** Provide typed bindings and a service wrapper for the `ProviderRegistry` contract (verified hospitals, clinics, labs, pharmacies, insurers, and authorized staff accounts).

**Tasks:**
- [ ] Generate/integrate Rust bindings for the contract interface.
- [ ] Implement a `ProviderRegistryService` with methods to register a provider, add/remove authorized staff, and query verification status.
- [ ] Sync provider state into the `providers`/`provider_staff` tables via the indexer (#20).
- [ ] Add contract-interaction tests against a local network.

**Acceptance Criteria:**
- Backend can register a provider and staff member on-chain and reflect verification status in the API within a test.


### Issue 16: Integrate ConsentAccessControl contract client — [GH #17](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/17)

**Labels:** backend, soroban

**Objective:** Provide typed bindings and a service wrapper for the `ConsentAccessControl` contract, which creates, approves, limits, expires, and revokes provider access permissions.

**Tasks:**
- [ ] Generate/integrate Rust bindings for the contract interface.
- [ ] Implement a `ConsentService` with methods to submit access requests, approve/limit/reject them, and revoke existing grants, via the XDR builder from #13.
- [ ] Enforce that only the owning patient's signed transaction can approve/revoke their own consent (validated on-chain, mirrored in API-level checks).
- [ ] Sync consent state into the `access_requests`/`consent_grants` tables via the indexer (#20).
- [ ] Add contract-interaction tests covering approval, time-limited expiry, and revocation.

**Acceptance Criteria:**
- A full request → approve → revoke cycle works end-to-end against a local Soroban network in a test.


### Issue 17: Integrate RecordCommitmentRegistry contract client — [GH #18](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/18)

**Labels:** backend, soroban

**Objective:** Provide typed bindings and a service wrapper for the `RecordCommitmentRegistry` contract, which stores hashes/commitments of encrypted records, categories, issuer references, and update events.

**Tasks:**
- [ ] Generate/integrate Rust bindings for the contract interface.
- [ ] Implement a `RecordCommitmentService` to submit a new record commitment (hash, category, issuer reference) and to look up commitment history for a patient.
- [ ] Ensure the commitment written on-chain matches the hash of the ciphertext stored in encrypted storage (#24) byte-for-byte.
- [ ] Add contract-interaction tests verifying commitment writes and reads.

**Acceptance Criteria:**
- A record's off-chain ciphertext hash matches the on-chain commitment for that record in an end-to-end test.


### Issue 18: Integrate DeviceAttestationRegistry contract client — [GH #19](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/19)

**Labels:** backend, soroban

**Objective:** Provide typed bindings and a service wrapper for the `DeviceAttestationRegistry` contract, covering approved medical devices/wearables and their revocation status.

**Tasks:**
- [ ] Generate/integrate Rust bindings for the contract interface.
- [ ] Implement a `DeviceRegistryService` to register/revoke device identities and query approval status.
- [ ] Sync device registration state into the `device_registrations` table via the indexer (#20).
- [ ] Add contract-interaction tests for registration and revocation.

**Acceptance Criteria:**
- Backend can register a device on-chain, mark it revoked, and have the API reflect the correct status.


### Issue 19: Integrate AuditEventEmitter contract client — [GH #20](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/20)

**Labels:** backend, soroban

**Objective:** Provide typed bindings and a service wrapper for the `AuditEventEmitter` contract, which emits events for access requests, consent grants/revocations, record additions, provider updates, and device attestations.

**Tasks:**
- [ ] Generate/integrate Rust bindings for the contract's event types.
- [ ] Ensure every other contract-writing service (#14-#18) triggers (directly or implicitly via the contract) a corresponding audit event.
- [ ] Define a canonical internal `AuditEvent` domain type that normalizes all emitted event variants for storage.
- [ ] Add tests asserting that key actions (consent approval, record addition, revocation) produce the expected audit event.

**Acceptance Criteria:**
- Every consent, record, and provider-affecting action produces a traceable audit event captured by the indexer.


### Issue 20: Build Soroban contract event indexer worker — [GH #21](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/21)

**Labels:** backend, soroban, infra

**Objective:** Build a resilient background worker that polls/streams Soroban `get-events` for all LockA contracts, decodes them, and updates the corresponding Postgres tables, so API reads never need to hit the chain directly.

**Tasks:**
- [ ] Implement a `Backend/worker` binary that polls `get-events` from the last-processed ledger, persisting a cursor/checkpoint in Postgres.
- [ ] Decode each event type from #14-#19 into domain events and dispatch to the relevant repository update logic.
- [ ] Make the worker idempotent and safe to restart from its last checkpoint without duplicating side effects.
- [ ] Add structured logging/metrics for indexing lag (current ledger vs. latest ledger).
- [ ] Add a Docker/compose entry and CI job running the worker against a local network with sample contract activity.

**Acceptance Criteria:**
- Worker recovers correctly after a restart, resuming from its checkpoint with no duplicate or missed events in a test scenario.
- Indexing lag is observable via logs/metrics.


---

## D. Core Domain Services & API


### Issue 21: Patient identity API endpoints — [GH #22](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/22)

**Labels:** backend, api

**Objective:** Expose REST endpoints for patient onboarding, profile management, and passport lookup, backed by #14 and #10.

**Tasks:**
- [ ] `POST /patients` — register a new patient passport (builds unsigned XDR per #13, links Stellar account to profile).
- [ ] `GET /patients/me` — fetch the authenticated patient's profile and passport status.
- [ ] `PATCH /patients/me` — update non-sensitive profile fields (emergency info, contact details).
- [ ] Enforce SEP-10 auth (#12) on all endpoints; a patient can only read/modify their own record.

**Acceptance Criteria:**
- Endpoints are covered by integration tests (#36) and documented in the OpenAPI spec (#33).


### Issue 22: Provider registry API endpoints — [GH #23](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/23)

**Labels:** backend, api

**Objective:** Expose REST endpoints for provider registration, staff management, and verification-status queries, backed by #15.

**Tasks:**
- [ ] `POST /providers` — register a new provider (pending verification).
- [ ] `GET /providers/{id}` — public verification status lookup.
- [ ] `POST /providers/{id}/staff` — add authorized staff accounts (provider-admin only).
- [ ] `DELETE /providers/{id}/staff/{staffId}` — remove staff access.
- [ ] Enforce role checks so only a provider's own admins can manage its staff.

**Acceptance Criteria:**
- Endpoints are covered by integration tests and documented in the OpenAPI spec.


### Issue 23: Consent & access-request API endpoints — [GH #24](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/24)

**Labels:** backend, api

**Objective:** Expose REST endpoints implementing the provider-requests-access / patient-approves-or-revokes flow, backed by #16.

**Tasks:**
- [ ] `POST /access-requests` — provider requests access to a record category for a patient, with purpose and requested duration.
- [ ] `GET /patients/me/access-requests` — patient lists pending/active/expired requests.
- [ ] `POST /access-requests/{id}/approve` (with optional limits/duration override) and `POST /access-requests/{id}/reject`.
- [ ] `POST /access-requests/{id}/revoke` — patient revokes an active grant.
- [ ] Ensure state transitions match on-chain consent state (rely on indexer #20 for the source of truth).

**Acceptance Criteria:**
- Full request/approve/revoke lifecycle is covered by integration tests, including expiry handling.


### Issue 24: Encrypted off-chain record storage service — [GH #25](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/25)

**Labels:** backend, security

**Objective:** Implement the service responsible for encrypting, storing, and retrieving medical record files/attachments in S3-compatible or IPFS-based object storage, decoupled from any single provider via a storage trait.

**Tasks:**
- [ ] Define a `RecordStorage` trait with methods `put_encrypted`, `get_encrypted`, `delete`.
- [ ] Implement an S3-compatible backend (via `aws-sdk-s3` or `object_store` crate) as the default implementation.
- [ ] Implement envelope encryption: per-record data key encrypted under a patient- or provider-scoped key; never store plaintext at rest.
- [ ] Compute and return the ciphertext hash used for on-chain commitment anchoring (feeds #17).
- [ ] Add tests using a local S3-compatible emulator (e.g. MinIO in docker-compose).

**Acceptance Criteria:**
- Uploaded records are unreadable at rest without the corresponding data key.
- Ciphertext hash returned by this service exactly matches what gets anchored on-chain.


### Issue 25: Record upload/retrieval API with on-chain commitment anchoring — [GH #26](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/26)

**Labels:** backend, api

**Objective:** Expose REST endpoints tying together encrypted storage (#24) and on-chain commitment anchoring (#17), so providers can add records and patients/authorized providers can retrieve them.

**Tasks:**
- [ ] `POST /patients/{id}/records` — provider uploads a record (encrypts via #24, anchors hash via #17, indexes metadata in `record_index`).
- [ ] `GET /patients/me/records` — patient lists their own records.
- [ ] `GET /records/{id}` — authorized provider or the owning patient retrieves and decrypts a record, enforcing active consent from #16/#23.
- [ ] Reject any retrieval where consent is missing, expired, or revoked, with a clear 403 and audit log entry.

**Acceptance Criteria:**
- A provider without valid consent cannot retrieve a patient's record, verified by an integration test.
- On-chain commitment always matches the stored ciphertext hash for every record created through this API.


### Issue 26: Device attestation & IoT reading ingestion API — [GH #27](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/27)

**Labels:** backend, api

**Objective:** Expose an ingestion endpoint for signed medical-device/wearable readings, verifying device signatures and patient consent before encrypting and anchoring them, per the documented IoT data flow.

**Tasks:**
- [ ] `POST /devices/{deviceId}/readings` — accepts a signed reading payload (device ID, timestamp, signature, payload).
- [ ] Verify the device's signature against its registered public key from #18; reject unknown/revoked devices.
- [ ] Verify patient consent for device-originated data before accepting the reading.
- [ ] Encrypt and store the reading via #24, and anchor a hash/attestation reference via #18/#17 as appropriate.

**Acceptance Criteria:**
- Readings from unregistered or revoked devices are rejected before any storage or chain interaction occurs.
- A valid signed reading flows end-to-end into encrypted storage with a corresponding on-chain reference.


### Issue 27: Audit log API (patient-facing access history) — [GH #28](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/28)

**Labels:** backend, api

**Objective:** Expose a read API letting patients see who requested access, who was granted access, and when records were added or updated, sourced from the `AuditEventEmitter` indexer data (#19/#20).

**Tasks:**
- [ ] `GET /patients/me/audit-log` — paginated, filterable (by date range, provider, event type) audit trail.
- [ ] Ensure the endpoint reads only from the indexed `audit_log_index` table, never live from chain.
- [ ] Add pagination and sensible default ordering (most recent first).

**Acceptance Criteria:**
- A patient can see a complete, correctly ordered history of access requests, approvals, revocations, and record updates in an integration test.


### Issue 28: Notification service (email/SMS/push) — [GH #29](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/29)

**Labels:** backend, api

**Objective:** Implement a notification service that informs patients and providers of access requests, approvals, revocations, and record updates via email/SMS/push, decoupled behind a provider-agnostic trait.

**Tasks:**
- [ ] Define a `Notifier` trait with pluggable backends (e.g. SMTP/SendGrid for email, Twilio for SMS, a push-notification provider).
- [ ] Trigger notifications from the relevant domain events (new access request, approval, revocation, new record) rather than directly from HTTP handlers.
- [ ] Make delivery asynchronous (queued) so a slow notification provider never blocks the request path.
- [ ] Add a no-op/logging backend for local development and tests.

**Acceptance Criteria:**
- Triggering a domain event (e.g. new access request) results in a queued notification job, verified in an integration test using the logging backend.


### Issue 29: Zero-knowledge proof verification service & API — [GH #30](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/30)

**Labels:** backend, api, security

**Objective:** Implement a modular ZK proof verification service supporting selective-disclosure proofs (e.g. vaccination status, insurance eligibility, provider authorization) for the MVP's one-or-two initial proof types, with room to add more circuits later.

**Tasks:**
- [ ] Define a `ZkVerifier` trait abstracting proof type, public inputs, and verification result, so the underlying proving system can evolve independently of the API.
- [ ] Implement verification for the first MVP proof type (e.g. vaccination proof) using the chosen proof system's Rust verifier.
- [ ] `POST /zk/verify` — accepts a proof + public inputs + claim type, returns a verification result without exposing underlying data.
- [ ] Persist verification outcomes (not the underlying private data) for audit purposes.

**Acceptance Criteria:**
- A valid proof for the supported claim type verifies successfully; a tampered or invalid proof is rejected, covered by tests.


### Issue 30: AI-assisted patient summary service & API — [GH #31](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/31)

**Labels:** backend, api

**Objective:** Implement a service that generates assistive patient-history summaries and missing-record/over-broad-access flags, strictly scoped to records the patient has consented to share, clearly labeled as non-diagnostic.

**Tasks:**
- [ ] Define an `AiSummaryProvider` trait so the underlying model/provider is swappable.
- [ ] `GET /patients/{id}/summary` — generates or returns a cached summary built only from consented, decrypted records for the requesting context.
- [ ] Label all AI output with a clear disclaimer that it is assistive only, not a diagnosis or medical advice.
- [ ] Add a flag/detector for access requests that ask for more record categories than the requested purpose typically requires (privacy workflow auditing).

**Acceptance Criteria:**
- Summary generation only ever includes data the requester is authorized to see, verified in a test using a restricted consent scope.
- Every AI-generated response includes the assistive-only disclaimer.


---

## E. Cross-Cutting Concerns


### Issue 31: API input validation & centralized error handling — [GH #32](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/32)

**Labels:** backend, api

**Objective:** Establish consistent request validation and a single error-handling strategy so every endpoint returns predictable, well-structured error responses.

**Tasks:**
- [ ] Adopt a validation approach for request DTOs (e.g. `validator` crate or manual `TryFrom` conversions) applied consistently across handlers.
- [ ] Define a single `ApiError` type mapping domain/repository/Soroban errors to HTTP status codes and a consistent JSON error body (code, message, optional field errors).
- [ ] Ensure no internal error details (SQL, stack traces, raw RPC errors) ever leak into API responses.
- [ ] Add tests asserting the error shape for common failure cases (validation, auth, not-found, conflict).

**Acceptance Criteria:**
- All endpoints return errors in the same JSON shape; no internal error string leaks are observed in tests.


### Issue 32: Rate limiting & abuse-protection middleware — [GH #33](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/33)

**Labels:** backend, security

**Objective:** Protect the API from abuse (credential stuffing on the SEP-10 flow, record-upload spam, scraping) with rate limiting and basic abuse detection.

**Tasks:**
- [ ] Add a rate-limiting middleware (e.g. `tower-governor` or Redis-backed token bucket) applied per-IP and per-authenticated-account.
- [ ] Apply stricter limits to sensitive endpoints (`/auth/challenge`, `/auth/verify`, record upload).
- [ ] Return standard `429` responses with `Retry-After` headers.
- [ ] Add tests verifying limits are enforced and reset correctly.

**Acceptance Criteria:**
- Exceeding the configured limit on a protected endpoint returns `429` with a `Retry-After` header, verified by a test.


### Issue 33: OpenAPI documentation generation (utoipa) published from code — [GH #34](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/34)

**Labels:** backend, documentation, api

**Objective:** Generate an always-up-to-date OpenAPI specification directly from the Rust route/handler definitions, and serve interactive docs.

**Tasks:**
- [ ] Add `utoipa` (and `utoipa-swagger-ui` or `utoipa-rapidoc`) annotations to all handlers and DTOs.
- [ ] Serve the generated spec at `/openapi.json` and interactive docs at `/docs`.
- [ ] Add a CI check that fails if the spec fails to generate (catches annotation drift).

**Acceptance Criteria:**
- `/docs` renders a complete, accurate interactive API reference matching the implemented endpoints.


### Issue 34: Observability: metrics & distributed tracing (OpenTelemetry) — [GH #35](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/35)

**Labels:** backend, infra

**Objective:** Instrument the service with metrics (request rates/latencies/errors, indexer lag, queue depth) and distributed tracing exported via OpenTelemetry, so production issues are diagnosable.

**Tasks:**
- [ ] Add `tracing-opentelemetry` and an OTLP exporter, configurable via environment variables.
- [ ] Expose a `/metrics` endpoint (Prometheus format) covering HTTP metrics, DB pool stats, and indexer lag from #20.
- [ ] Add example Grafana/Prometheus config (or docs) for local observability stacks in docker-compose.

**Acceptance Criteria:**
- `/metrics` exposes request counts/latencies and indexer lag; traces can be exported to a local OTLP collector in dev.


---

## F. Testing


### Issue 35: Unit test suite conventions & coverage reporting — [GH #36](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/36)

**Labels:** backend, testing

**Objective:** Establish project-wide conventions for unit tests (naming, module layout, mocking strategy) and wire up coverage reporting.

**Tasks:**
- [ ] Document unit-testing conventions (e.g. `#[cfg(test)] mod tests` per module, trait-based mocking for external dependencies) in `Backend/README.md` or a `CONTRIBUTING.md`.
- [ ] Add `cargo-llvm-cov` (or `tarpaulin`) to generate coverage reports.
- [ ] Add a CI job publishing coverage as a build artifact / PR comment.
- [ ] Set an initial coverage baseline/threshold for core domain and service crates.

**Acceptance Criteria:**
- Coverage report is generated in CI and available as an artifact on every PR.


### Issue 36: Integration test suite with testcontainers (Postgres) — [GH #37](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/37)

**Labels:** backend, testing

**Objective:** Add integration tests that exercise the real Postgres-backed repository and API layers against an ephemeral database, avoiding mocked SQL.

**Tasks:**
- [ ] Add `testcontainers` (Rust) to spin up an ephemeral Postgres instance per test run/module.
- [ ] Run migrations from #9 against the ephemeral database before each test suite.
- [ ] Write integration tests for the repository layer (#10) and the patient/provider/consent/record API endpoints (#21-#23, #25, #27) using an in-process Axum test server.
- [ ] Wire this test suite into CI (#5) as a separate job/stage.

**Acceptance Criteria:**
- Integration tests run against a real, ephemeral Postgres instance in CI with no shared state between test runs.


### Issue 37: Contract-interaction test suite against local Soroban network — [GH #38](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/38)

**Labels:** backend, testing, soroban

**Objective:** Add a dedicated test suite that deploys the LockA Soroban contracts to a local network (per #6) and exercises every contract-client integration from Epic C against real contract execution, not mocks.

**Tasks:**
- [ ] Add tooling to deploy/initialize the required contracts (coordinating with the `locka-contracts` repo) into the local Soroban network before tests run.
- [ ] Write contract-interaction tests for #14-#19: identity registration, provider registration, consent lifecycle, record commitment anchoring, device attestation, and audit events.
- [ ] Run this suite as its own CI job (can be slower/heavier than unit/integration tests), gated on changes touching `Backend/soroban/**`.
- [ ] Document how to run this suite locally against `docker compose`'s Soroban network.

**Acceptance Criteria:**
- All contract-client integrations are verified against real contract execution on a local network, both locally and in CI.


### Issue 38: End-to-end API test suite — [GH #39](https://github.com/LockA-Medical-Passport/LockA-Backend/issues/39)

**Labels:** backend, testing

**Objective:** Add black-box end-to-end tests that run the fully assembled service (API + worker + Postgres + local Soroban network, e.g. via docker-compose) and drive it purely through its public HTTP API, validating the key user flows from the platform documentation.

**Tasks:**
- [ ] Set up an E2E test harness that boots the docker-compose stack from #6 and waits for readiness.
- [ ] Implement E2E scenarios: patient onboarding, provider requests access → patient approves → provider retrieves record, patient revokes access → provider access denied, device reading ingestion, audit log reflects all of the above.
- [ ] Run this suite in CI on a schedule and/or before releases (it is slower than unit/integration tests).
- [ ] Document how to run the E2E suite locally.

**Acceptance Criteria:**
- All key user flows in the E2E scenario list pass against the fully assembled stack.

