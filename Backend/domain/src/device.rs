use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::RepoError;

/// Mirrors the `device_type` Postgres enum. Contract-defined vocabulary —
/// extend in lockstep with `DeviceAttestationRegistry`. Deliberately coarse:
/// see the PII minimization review in `docs/schema.md` for why manufacturer
/// and model are excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "device_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Glucometer,
    BloodPressureMonitor,
    PulseOximeter,
    Wearable,
    Scale,
    Thermometer,
}

/// Mirrors the `device_status` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "device_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Active,
    Revoked,
}

/// Read model of `DeviceAttestationRegistry`. `device_public_key` is
/// chain-sourced so a compromised backend can't silently swap in its own
/// key and forge readings.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct DeviceRegistration {
    pub id: Uuid,
    pub chain_device_id: String,
    pub patient_id: Uuid,
    pub device_public_key: Vec<u8>,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub registered_ledger: i64,
    pub registered_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct DeviceRegistrationChainUpdate {
    pub chain_device_id: String,
    pub patient_id: Uuid,
    pub device_public_key: Vec<u8>,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub registered_ledger: i64,
    pub registered_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
}

/// Pointer to an encrypted device reading. Same shape as
/// [`crate::record::RecordIndexEntry`], kept separate because readings
/// arrive at far higher volume and carry their own verification state.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct DeviceReadingIndexEntry {
    pub id: Uuid,
    pub device_registration_id: Uuid,
    pub patient_id: Uuid,
    pub record_index_id: Option<Uuid>,
    pub recorded_at: OffsetDateTime,
    pub ingested_at: OffsetDateTime,
    pub signature_verified: bool,
    pub ciphertext_sha256: Vec<u8>,
    pub storage_backend: crate::record::StorageBackend,
    pub storage_uri: String,
    pub encrypted_data_key: Vec<u8>,
    pub key_encryption_key_id: String,
}

/// Everything known once a signed reading (#26) has been verified and
/// encrypted. `signature_verified` should always be `true` in practice —
/// #26 rejects bad signatures before storage — but is carried as data
/// rather than assumed, so the invariant is auditable in SQL.
#[derive(Debug, Clone)]
pub struct NewDeviceReadingIndexEntry {
    pub device_registration_id: Uuid,
    pub patient_id: Uuid,
    pub recorded_at: OffsetDateTime,
    pub signature_verified: bool,
    pub ciphertext_sha256: Vec<u8>,
    pub storage_backend: crate::record::StorageBackend,
    pub storage_uri: String,
    pub encrypted_data_key: Vec<u8>,
    pub key_encryption_key_id: String,
}

// See the comment on `PatientRepository` in patient.rs — same reasoning
// applies to every trait in this crate.
#[allow(async_fn_in_trait)]
pub trait DeviceRepository {
    /// Upserts keyed on `chain_device_id`, called by the indexer whenever
    /// `DeviceAttestationRegistry` emits a relevant event.
    async fn upsert_registration_from_chain(
        &self,
        update: DeviceRegistrationChainUpdate,
    ) -> Result<DeviceRegistration, RepoError>;

    async fn find_registration_by_chain_device_id(
        &self,
        chain_device_id: &str,
    ) -> Result<Option<DeviceRegistration>, RepoError>;

    /// Fails with [`RepoError::Conflict`] if `ciphertext_sha256` already
    /// exists — doubles as replay protection for a resubmitted signed
    /// reading.
    async fn insert_reading(
        &self,
        new: NewDeviceReadingIndexEntry,
    ) -> Result<DeviceReadingIndexEntry, RepoError>;

    /// Newest first.
    async fn list_readings_for_patient(
        &self,
        patient_id: Uuid,
    ) -> Result<Vec<DeviceReadingIndexEntry>, RepoError>;
}
