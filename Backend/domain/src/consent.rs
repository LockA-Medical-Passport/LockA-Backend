use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::RepoError;

/// Mirrors the `record_category` Postgres enum. Contract-defined vocabulary
/// — extend in lockstep with `ConsentAccessControl`. See `docs/schema.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "record_category", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RecordCategory {
    Consultation,
    Laboratory,
    Imaging,
    Prescription,
    Immunization,
    DeviceReading,
    EmergencyProfile,
    Insurance,
}

/// Mirrors the `purpose_code` Postgres enum. A closed vocabulary by design
/// — see the PII minimization review in `docs/schema.md` for why this is
/// never free text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "purpose_code", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PurposeCode {
    Treatment,
    Emergency,
    Referral,
    LaboratoryProcessing,
    PrescriptionFulfilment,
    InsuranceClaim,
    PublicHealthReporting,
}

/// Mirrors the `access_request_status` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "access_request_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AccessRequestStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
    Withdrawn,
}

/// Read model of the request half of `ConsentAccessControl`. Entirely
/// chain-sourced besides the FK columns, which the indexer resolves from
/// the event's account ids.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct AccessRequest {
    pub id: Uuid,
    pub chain_request_id: String,
    pub patient_id: Uuid,
    pub provider_id: Uuid,
    pub requested_by_staff_id: Option<Uuid>,
    pub record_category: RecordCategory,
    pub purpose_code: PurposeCode,
    pub requested_duration_secs: i32,
    pub status: AccessRequestStatus,
    pub requested_ledger: i64,
    pub requested_at: OffsetDateTime,
    pub resolved_ledger: Option<i64>,
    pub resolved_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct AccessRequestChainUpdate {
    pub chain_request_id: String,
    pub patient_id: Uuid,
    pub provider_id: Uuid,
    pub requested_by_staff_id: Option<Uuid>,
    pub record_category: RecordCategory,
    pub purpose_code: PurposeCode,
    pub requested_duration_secs: i32,
    pub status: AccessRequestStatus,
    pub requested_ledger: i64,
    pub requested_at: OffsetDateTime,
    pub resolved_ledger: Option<i64>,
    pub resolved_at: Option<OffsetDateTime>,
}

/// Read model of the grant half of `ConsentAccessControl` — the authority
/// for "may this provider decrypt this category of this patient's records
/// right now?" No `status` column by design: active iff
/// `revoked_at IS NULL AND expires_at > now()`. See `docs/schema.md`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct ConsentGrant {
    pub id: Uuid,
    pub chain_grant_id: String,
    pub access_request_id: Option<Uuid>,
    pub patient_id: Uuid,
    pub provider_id: Uuid,
    pub record_category: RecordCategory,
    pub granted_ledger: i64,
    pub granted_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
    pub revoked_ledger: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ConsentGrantChainUpdate {
    pub chain_grant_id: String,
    pub access_request_id: Option<Uuid>,
    pub patient_id: Uuid,
    pub provider_id: Uuid,
    pub record_category: RecordCategory,
    pub granted_ledger: i64,
    pub granted_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
    pub revoked_ledger: Option<i64>,
}

// See the comment on `PatientRepository` in patient.rs — same reasoning
// applies to every trait in this crate.
#[allow(async_fn_in_trait)]
pub trait ConsentRepository {
    /// Upserts keyed on `chain_request_id`, called by the indexer whenever
    /// `ConsentAccessControl` emits a request-related event.
    async fn upsert_access_request_from_chain(
        &self,
        update: AccessRequestChainUpdate,
    ) -> Result<AccessRequest, RepoError>;

    /// Upserts keyed on `chain_grant_id`, called by the indexer whenever
    /// `ConsentAccessControl` emits a grant-related event.
    async fn upsert_consent_grant_from_chain(
        &self,
        update: ConsentGrantChainUpdate,
    ) -> Result<ConsentGrant, RepoError>;

    /// Newest first — serves `GET /patients/me/access-requests` (#23).
    async fn list_access_requests_for_patient(
        &self,
        patient_id: Uuid,
        status: Option<AccessRequestStatus>,
    ) -> Result<Vec<AccessRequest>, RepoError>;

    /// The grant (if any) that currently authorizes `provider_id` to read
    /// `category` for `patient_id` — i.e. `revoked_at IS NULL AND
    /// expires_at > now()`. This is the check the record-retrieval path
    /// (#25) and device-reading ingestion (#26) both need before decrypting
    /// anything.
    async fn active_grant(
        &self,
        patient_id: Uuid,
        provider_id: Uuid,
        category: RecordCategory,
    ) -> Result<Option<ConsentGrant>, RepoError>;
}
