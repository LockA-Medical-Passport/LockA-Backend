use time::OffsetDateTime;
use uuid::Uuid;

use crate::consent::RecordCategory;
use crate::error::RepoError;

/// Mirrors the `storage_backend` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "storage_backend", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum StorageBackend {
    S3,
    Ipfs,
}

/// Metadata and pointers for an encrypted record. No raw medical content —
/// see `docs/schema.md`'s PII minimization review.
///
/// `ciphertext_sha256` does double duty as both the stored ciphertext's hash
/// and the value anchored on-chain by `RecordCommitmentRegistry`: one column
/// so the two cannot drift apart.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct RecordIndexEntry {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub ciphertext_sha256: Vec<u8>,
    pub record_category: RecordCategory,
    pub issuer_provider_id: Option<Uuid>,
    pub issued_by_staff_id: Option<Uuid>,
    pub storage_backend: StorageBackend,
    pub storage_uri: String,
    pub ciphertext_size_bytes: i64,
    pub encrypted_data_key: Vec<u8>,
    pub key_encryption_key_id: String,
    pub anchored_ledger: Option<i64>,
    pub anchored_at: Option<OffsetDateTime>,
    pub superseded_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
}

/// Everything known at upload time (#25), before the on-chain commitment
/// anchoring that follows it has landed — `anchored_ledger`/`anchored_at`
/// start `NULL` and are filled in by [`RecordIndexRepository::mark_anchored`].
#[derive(Debug, Clone)]
pub struct NewRecordIndexEntry {
    pub patient_id: Uuid,
    pub ciphertext_sha256: Vec<u8>,
    pub record_category: RecordCategory,
    pub issuer_provider_id: Option<Uuid>,
    pub issued_by_staff_id: Option<Uuid>,
    pub storage_backend: StorageBackend,
    pub storage_uri: String,
    pub ciphertext_size_bytes: i64,
    pub encrypted_data_key: Vec<u8>,
    pub key_encryption_key_id: String,
}

// See the comment on `PatientRepository` in patient.rs — same reasoning
// applies to every trait in this crate.
#[allow(async_fn_in_trait)]
pub trait RecordIndexRepository {
    /// The API-driven creation of a record (#25). Fails with
    /// [`RepoError::Conflict`] if `ciphertext_sha256` already exists —
    /// the same ciphertext can't be indexed twice.
    async fn insert(&self, new: NewRecordIndexEntry) -> Result<RecordIndexEntry, RepoError>;

    /// Applies the `RecordCommitmentRegistry` anchoring event once observed
    /// by the indexer, confirming the on-chain commitment matches this
    /// row's `ciphertext_sha256`.
    async fn mark_anchored(
        &self,
        id: Uuid,
        anchored_ledger: i64,
        anchored_at: OffsetDateTime,
    ) -> Result<RecordIndexEntry, RepoError>;

    /// Newest first — serves `GET /patients/me/records` (#25).
    async fn list_for_patient(
        &self,
        patient_id: Uuid,
        category: Option<RecordCategory>,
    ) -> Result<Vec<RecordIndexEntry>, RepoError>;

    async fn find_by_ciphertext_hash(
        &self,
        ciphertext_sha256: &[u8],
    ) -> Result<Option<RecordIndexEntry>, RepoError>;
}
