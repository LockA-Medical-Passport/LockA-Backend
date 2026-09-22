use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::RepoError;

/// Mirrors the `patient_status` Postgres enum. See `docs/schema.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "patient_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PatientStatus {
    Active,
    Suspended,
    Recovering,
}

/// Read model of `PatientIdentityRegistry`. Every non-derived field here is
/// chain-sourced — there is no patient-owned API write path onto this table,
/// by design (see `docs/schema.md`'s PII minimization review).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct Patient {
    pub id: Uuid,
    pub stellar_account_id: String,
    pub passport_id: String,
    pub identity_commitment: Vec<u8>,
    pub recovery_config_hash: Option<Vec<u8>>,
    pub status: PatientStatus,
    pub registered_ledger: i64,
    pub registered_at: OffsetDateTime,
    pub updated_ledger: i64,
    pub first_indexed_at: OffsetDateTime,
}

/// The fields the indexer supplies when `PatientIdentityRegistry` emits an
/// event for this account. `id` and `first_indexed_at` are assigned by the
/// repository, not the caller.
#[derive(Debug, Clone)]
pub struct PatientChainUpdate {
    pub stellar_account_id: String,
    pub passport_id: String,
    pub identity_commitment: Vec<u8>,
    pub recovery_config_hash: Option<Vec<u8>>,
    pub status: PatientStatus,
    pub registered_ledger: i64,
    pub registered_at: OffsetDateTime,
    pub updated_ledger: i64,
}

// Call sites always use a concrete `impl PatientRepository`, monomorphized —
// never `dyn PatientRepository` — so the `Send`-bound concern this lint
// warns about (which only bites generic/dyn contexts) doesn't apply: the
// concrete future underneath is `Send` in practice because `sqlx`'s own
// futures are.
#[allow(async_fn_in_trait)]
pub trait PatientRepository {
    /// Inserts a new patient, or updates an existing one keyed on
    /// `stellar_account_id`. Called by the indexer (#21) whenever
    /// `PatientIdentityRegistry` emits a relevant event.
    ///
    /// Idempotent under replay in both directions: applying the same
    /// update twice, or an update carrying an `updated_ledger` no newer
    /// than what's already stored, leaves the row as the newest state seen
    /// so far — a replayed *older* event can never regress a patient's
    /// status past a newer one already applied.
    async fn upsert_from_chain(&self, update: PatientChainUpdate) -> Result<Patient, RepoError>;

    async fn find_by_stellar_account_id(
        &self,
        stellar_account_id: &str,
    ) -> Result<Option<Patient>, RepoError>;

    async fn find_by_passport_id(&self, passport_id: &str) -> Result<Option<Patient>, RepoError>;
}
