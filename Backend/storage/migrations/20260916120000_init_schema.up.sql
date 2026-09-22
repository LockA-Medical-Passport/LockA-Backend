-- Initial schema, transcribed from Backend/docs/schema.md. That document is
-- the source of truth for *why*; this file exists to make the two match
-- exactly, not to make new design decisions. Two places below where the
-- design doc is ambiguous or leaves something as a recommendation rather
-- than a requirement are called out inline.
--
-- Primary keys are UUIDv7, generated in Rust (see the `uuid` crate usage in
-- the `storage` crate) rather than in SQL — hence no `DEFAULT` on any `id`
-- column. See "Conventions -> Primary keys" in schema.md.

CREATE EXTENSION IF NOT EXISTS citext;

-- --- Enumerations -----------------------------------------------------------
-- Contract-defined vocabularies (record_category, purpose_code, provider_type,
-- staff_role, device_type, audit_event_type, access_request_status) must be
-- extended in lockstep with the corresponding Soroban contract — see
-- schema.md's "Open questions for review", item 1.

CREATE TYPE patient_status AS ENUM ('active', 'suspended', 'recovering');

CREATE TYPE provider_type AS ENUM ('hospital', 'clinic', 'laboratory', 'pharmacy', 'insurer');

CREATE TYPE verification_status AS ENUM ('pending', 'verified', 'suspended', 'revoked');

CREATE TYPE staff_role AS ENUM ('admin', 'clinician', 'technician');

CREATE TYPE staff_status AS ENUM ('active', 'removed');

CREATE TYPE access_request_status AS ENUM (
    'pending', 'approved', 'rejected', 'expired', 'withdrawn'
);

CREATE TYPE record_category AS ENUM (
    'consultation', 'laboratory', 'imaging', 'prescription', 'immunization',
    'device_reading', 'emergency_profile', 'insurance'
);

CREATE TYPE purpose_code AS ENUM (
    'treatment', 'emergency', 'referral', 'laboratory_processing',
    'prescription_fulfilment', 'insurance_claim', 'public_health_reporting'
);

CREATE TYPE storage_backend AS ENUM ('s3', 'ipfs');

CREATE TYPE device_type AS ENUM (
    'glucometer', 'blood_pressure_monitor', 'pulse_oximeter', 'wearable',
    'scale', 'thermometer'
);

CREATE TYPE device_status AS ENUM ('active', 'revoked');

CREATE TYPE notification_event_type AS ENUM (
    'access_requested', 'access_approved', 'access_rejected', 'access_revoked',
    'access_expiring', 'record_added', 'device_reading_rejected', 'provider_verified'
);

CREATE TYPE notification_channel AS ENUM ('email', 'sms', 'push', 'whatsapp');

CREATE TYPE notification_status AS ENUM ('queued', 'sending', 'sent', 'failed', 'dead');

CREATE TYPE audit_event_type AS ENUM (
    'patient_registered', 'provider_registered', 'provider_verified',
    'staff_authorized', 'staff_removed', 'access_requested', 'access_approved',
    'access_rejected', 'access_revoked', 'record_anchored', 'record_superseded',
    'device_registered', 'device_revoked'
);

-- --- patients ----------------------------------------------------------------
-- Read model of PatientIdentityRegistry. One row per registered passport.

CREATE TABLE patients (
    id UUID PRIMARY KEY,
    stellar_account_id TEXT NOT NULL UNIQUE
        CHECK (char_length(stellar_account_id) = 56 AND stellar_account_id LIKE 'G%'),
    passport_id TEXT NOT NULL UNIQUE,
    identity_commitment BYTEA NOT NULL,
    recovery_config_hash BYTEA,
    status patient_status NOT NULL,
    registered_ledger BIGINT NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL,
    updated_ledger BIGINT NOT NULL,
    first_indexed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- --- providers -----------------------------------------------------------------
-- Read model of ProviderRegistry. Organizations, not people.

CREATE TABLE providers (
    id UUID PRIMARY KEY,
    stellar_account_id TEXT NOT NULL UNIQUE
        CHECK (char_length(stellar_account_id) = 56 AND stellar_account_id LIKE 'G%'),
    chain_provider_id TEXT NOT NULL UNIQUE,
    provider_type provider_type NOT NULL,
    verification_status verification_status NOT NULL,
    verified_ledger BIGINT,
    legal_name TEXT NOT NULL,
    display_name TEXT,
    -- schema.md's table for `providers` marks this "ISO-3166-1 CHECK" without
    -- stating NOT NULL either way (unlike the columns it explicitly marks
    -- NULL). Left nullable here as the more conservative reading; flagged in
    -- the PR for confirmation rather than silently deciding NOT NULL.
    country_code CHAR(2) CHECK (country_code = upper(country_code)),
    contact_email CITEXT,
    registered_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_providers_verification_status ON providers (verification_status);

-- --- provider_staff ------------------------------------------------------------
-- Individual accounts a provider has authorized to act on its behalf.

CREATE TABLE provider_staff (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES providers (id) ON DELETE CASCADE,
    stellar_account_id TEXT NOT NULL
        CHECK (char_length(stellar_account_id) = 56 AND stellar_account_id LIKE 'G%'),
    role staff_role NOT NULL,
    status staff_status NOT NULL,
    authorized_ledger BIGINT NOT NULL,
    removed_at TIMESTAMPTZ,
    -- Deliberately not a global UNIQUE on stellar_account_id: a clinician
    -- doing locum work at two clinics needs two rows. See schema.md.
    UNIQUE (provider_id, stellar_account_id)
);

CREATE INDEX idx_provider_staff_stellar_account_id ON provider_staff (stellar_account_id);

-- --- access_requests -----------------------------------------------------------
-- Read model of the request half of ConsentAccessControl.

CREATE TABLE access_requests (
    id UUID PRIMARY KEY,
    chain_request_id TEXT NOT NULL UNIQUE,
    patient_id UUID NOT NULL REFERENCES patients (id),
    provider_id UUID NOT NULL REFERENCES providers (id),
    requested_by_staff_id UUID REFERENCES provider_staff (id),
    record_category record_category NOT NULL,
    purpose_code purpose_code NOT NULL,
    requested_duration_secs INTEGER NOT NULL CHECK (requested_duration_secs > 0),
    status access_request_status NOT NULL,
    requested_ledger BIGINT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL,
    resolved_ledger BIGINT,
    resolved_at TIMESTAMPTZ
);

CREATE INDEX idx_access_requests_patient_status_requested_at
    ON access_requests (patient_id, status, requested_at DESC);
CREATE INDEX idx_access_requests_provider_status
    ON access_requests (provider_id, status);

-- --- consent_grants --------------------------------------------------------------
-- Read model of the grant half of ConsentAccessControl. The authority for
-- "may this provider decrypt this category of this patient's records now?"

CREATE TABLE consent_grants (
    id UUID PRIMARY KEY,
    chain_grant_id TEXT NOT NULL UNIQUE,
    access_request_id UUID REFERENCES access_requests (id),
    patient_id UUID NOT NULL REFERENCES patients (id),
    provider_id UUID NOT NULL REFERENCES providers (id),
    record_category record_category NOT NULL,
    granted_ledger BIGINT NOT NULL,
    granted_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revoked_ledger BIGINT,
    CHECK (expires_at > granted_at),
    CHECK (revoked_at IS NULL OR revoked_at >= granted_at)
);

-- No `status` column by design: a grant is active iff
-- `revoked_at IS NULL AND expires_at > now()`. See schema.md.
CREATE INDEX idx_consent_grants_active
    ON consent_grants (patient_id, provider_id, record_category)
    WHERE revoked_at IS NULL;

-- --- record_index ----------------------------------------------------------------
-- Metadata and pointers for encrypted records. No raw medical content lives
-- here or anywhere else in this schema — see schema.md's PII review.

CREATE TABLE record_index (
    id UUID PRIMARY KEY,
    patient_id UUID NOT NULL REFERENCES patients (id),
    -- Both the hash of the stored ciphertext and the value anchored on-chain
    -- by RecordCommitmentRegistry — one column so the two cannot drift.
    ciphertext_sha256 BYTEA NOT NULL UNIQUE CHECK (octet_length(ciphertext_sha256) = 32),
    record_category record_category NOT NULL,
    issuer_provider_id UUID REFERENCES providers (id),
    issued_by_staff_id UUID REFERENCES provider_staff (id),
    storage_backend storage_backend NOT NULL,
    storage_uri TEXT NOT NULL,
    ciphertext_size_bytes BIGINT NOT NULL CHECK (ciphertext_size_bytes > 0),
    -- The per-record data key, wrapped under a KEK. The KEK itself must never
    -- be in Postgres. See schema.md.
    encrypted_data_key BYTEA NOT NULL,
    key_encryption_key_id TEXT NOT NULL,
    anchored_ledger BIGINT,
    anchored_at TIMESTAMPTZ,
    superseded_by_id UUID REFERENCES record_index (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_record_index_patient_category_created
    ON record_index (patient_id, record_category, created_at DESC);
CREATE INDEX idx_record_index_issuer_created
    ON record_index (issuer_provider_id, created_at DESC);
CREATE INDEX idx_record_index_unanchored
    ON record_index (patient_id) WHERE anchored_ledger IS NULL;

-- --- device_registrations ----------------------------------------------------------
-- Read model of DeviceAttestationRegistry.

CREATE TABLE device_registrations (
    id UUID PRIMARY KEY,
    chain_device_id TEXT NOT NULL UNIQUE,
    patient_id UUID NOT NULL REFERENCES patients (id),
    device_public_key BYTEA NOT NULL CHECK (octet_length(device_public_key) = 32),
    device_type device_type NOT NULL,
    status device_status NOT NULL,
    registered_ledger BIGINT NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CHECK ((status = 'revoked') = (revoked_at IS NOT NULL))
);

CREATE INDEX idx_device_registrations_patient_status ON device_registrations (patient_id, status);

-- --- device_readings_index ------------------------------------------------------------
-- Pointers to encrypted device readings. Same shape as record_index, kept
-- separate because readings arrive at far higher volume.

CREATE TABLE device_readings_index (
    id UUID PRIMARY KEY,
    device_registration_id UUID NOT NULL REFERENCES device_registrations (id),
    -- Denormalized from device_registrations: device ownership never changes
    -- in place (reassignment is revoke + register), so this is safe to cache.
    patient_id UUID NOT NULL REFERENCES patients (id),
    record_index_id UUID REFERENCES record_index (id),
    recorded_at TIMESTAMPTZ NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    signature_verified BOOLEAN NOT NULL,
    -- UNIQUE doubles as replay protection for signed readings.
    ciphertext_sha256 BYTEA NOT NULL UNIQUE CHECK (octet_length(ciphertext_sha256) = 32),
    storage_backend storage_backend NOT NULL,
    storage_uri TEXT NOT NULL,
    encrypted_data_key BYTEA NOT NULL,
    key_encryption_key_id TEXT NOT NULL
);

CREATE INDEX idx_device_readings_index_patient_recorded
    ON device_readings_index (patient_id, recorded_at DESC);

-- --- notification_channels --------------------------------------------------------------
-- Every piece of direct-contact PII in the system lives in this one table,
-- so access control, encryption, and retention have exactly one place to
-- apply. See schema.md, "Tables added beyond the issue's list".

CREATE TABLE notification_channels (
    id UUID PRIMARY KEY,
    patient_id UUID REFERENCES patients (id) ON DELETE CASCADE,
    provider_id UUID REFERENCES providers (id) ON DELETE CASCADE,
    channel notification_channel NOT NULL,
    destination TEXT NOT NULL,
    verified_at TIMESTAMPTZ,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (num_nonnulls(patient_id, provider_id) = 1),
    UNIQUE (patient_id, channel, destination),
    UNIQUE (provider_id, channel, destination)
);

-- --- notifications ------------------------------------------------------------------
-- Outbound notification jobs and delivery state. Off-chain only.

CREATE TABLE notifications (
    id UUID PRIMARY KEY,
    patient_id UUID REFERENCES patients (id),
    provider_id UUID REFERENCES providers (id),
    event_type notification_event_type NOT NULL,
    channel notification_channel NOT NULL,
    status notification_status NOT NULL,
    -- Derived from the triggering event's identity, e.g.
    -- "access_approved:{chain_grant_id}:{channel}" — makes delivery
    -- idempotent under indexer replay.
    dedupe_key TEXT NOT NULL UNIQUE,
    access_request_id UUID REFERENCES access_requests (id),
    consent_grant_id UUID REFERENCES consent_grants (id),
    record_index_id UUID REFERENCES record_index (id),
    attempts SMALLINT NOT NULL DEFAULT 0,
    queued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    -- The transport's error only. Must never be allowed to contain the
    -- rendered message body — see schema.md's PII review.
    last_error TEXT,
    CHECK (num_nonnulls(patient_id, provider_id) = 1)
);

CREATE INDEX idx_notifications_dispatch_queue
    ON notifications (status, queued_at)
    WHERE status IN ('queued', 'sending');

-- --- audit_log_index -----------------------------------------------------------------
-- Read model of AuditEventEmitter. Append-only: no UPDATE or DELETE should
-- ever be issued against this table. schema.md recommends enforcing that
-- with a role-level GRANT of SELECT, INSERT only, via separate api/worker
-- database roles — deferred, since standing up those roles is infrastructure
-- beyond "implement the schema", not a schema decision itself.

CREATE TABLE audit_log_index (
    id UUID PRIMARY KEY,
    ledger_sequence BIGINT NOT NULL,
    transaction_hash BYTEA NOT NULL CHECK (octet_length(transaction_hash) = 32),
    event_index INTEGER NOT NULL,
    event_type audit_event_type NOT NULL,
    actor_account_id TEXT NOT NULL
        CHECK (char_length(actor_account_id) = 56 AND actor_account_id LIKE 'G%'),
    subject_account_id TEXT
        CHECK (
            subject_account_id IS NULL
            OR (char_length(subject_account_id) = 56 AND subject_account_id LIKE 'G%')
        ),
    patient_id UUID REFERENCES patients (id),
    provider_id UUID REFERENCES providers (id),
    access_request_id UUID REFERENCES access_requests (id),
    consent_grant_id UUID REFERENCES consent_grants (id),
    record_index_id UUID REFERENCES record_index (id),
    occurred_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL,
    -- Natural key from the ledger: lets the indexer `ON CONFLICT DO NOTHING`
    -- and safely replay any ledger range after a restart.
    UNIQUE (transaction_hash, event_index)
);

CREATE INDEX idx_audit_log_index_patient_timeline
    ON audit_log_index (patient_id, occurred_at DESC, ledger_sequence DESC, event_index DESC);
CREATE INDEX idx_audit_log_index_event_type_occurred
    ON audit_log_index (event_type, occurred_at DESC);

-- --- indexer_checkpoints ---------------------------------------------------------------
-- The indexer worker's resume cursor. Required by issue #21, which has
-- nowhere else in the schema to persist it.

CREATE TABLE indexer_checkpoints (
    stream_name TEXT PRIMARY KEY,
    contract_id TEXT NOT NULL,
    last_processed_ledger BIGINT NOT NULL,
    last_processed_event_index INTEGER NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
