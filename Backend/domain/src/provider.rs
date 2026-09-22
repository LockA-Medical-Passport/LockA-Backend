use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::RepoError;

/// Mirrors the `provider_type` Postgres enum. Contract-defined vocabulary —
/// extend in lockstep with `ProviderRegistry`. See `docs/schema.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "provider_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Hospital,
    Clinic,
    Laboratory,
    Pharmacy,
    Insurer,
}

/// Mirrors the `verification_status` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "verification_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Pending,
    Verified,
    Suspended,
    Revoked,
}

/// Mirrors the `staff_role` Postgres enum. Contract-defined vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "staff_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum StaffRole {
    Admin,
    Clinician,
    Technician,
}

/// Mirrors the `staff_status` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "staff_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum StaffStatus {
    Active,
    Removed,
}

/// Read model of `ProviderRegistry`. Organizations, not people.
///
/// Unlike `patients`, this row genuinely mixes provenance: `legal_name`,
/// `display_name`, `country_code`, and `contact_email` are API-submitted at
/// registration (#22), while the rest is chain-sourced. Both halves must be
/// present for every `NOT NULL` column from the moment the row exists — see
/// [`ProviderRepository::register`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct Provider {
    pub id: Uuid,
    pub stellar_account_id: String,
    pub chain_provider_id: String,
    pub provider_type: ProviderType,
    pub verification_status: VerificationStatus,
    pub verified_ledger: Option<i64>,
    pub legal_name: String,
    pub display_name: Option<String>,
    pub country_code: Option<String>,
    pub contact_email: Option<String>,
    pub registered_at: OffsetDateTime,
}

/// Everything needed to create a provider row. Assembled by the #22 API
/// handler once the provider's on-chain registration has been confirmed
/// (so `chain_provider_id` and `registered_at` are already known) — the
/// repository layer doesn't model the multi-step registration workflow
/// itself, only the row it ends in.
#[derive(Debug, Clone)]
pub struct NewProvider {
    pub stellar_account_id: String,
    pub chain_provider_id: String,
    pub provider_type: ProviderType,
    pub verification_status: VerificationStatus,
    pub legal_name: String,
    pub display_name: Option<String>,
    pub country_code: Option<String>,
    pub contact_email: Option<String>,
    pub registered_at: OffsetDateTime,
}

// See the comment on `PatientRepository` in patient.rs — same reasoning
// applies to every trait in this crate.
#[allow(async_fn_in_trait)]
pub trait ProviderRepository {
    /// The initial insert of a provider row. Fails with
    /// [`RepoError::Conflict`] if `stellar_account_id` or
    /// `chain_provider_id` is already registered.
    async fn register(&self, new: NewProvider) -> Result<Provider, RepoError>;

    /// Applies a `ProviderRegistry` verification-status change from the
    /// indexer. `verification_status` and `verified_ledger` are the only
    /// fields that legitimately change after a provider is created — the
    /// rest (account, type, registration) is set once at `register` time.
    async fn update_verification_status_from_chain(
        &self,
        chain_provider_id: &str,
        verification_status: VerificationStatus,
        verified_ledger: i64,
    ) -> Result<Provider, RepoError>;

    async fn find_by_stellar_account_id(
        &self,
        stellar_account_id: &str,
    ) -> Result<Option<Provider>, RepoError>;

    async fn find_by_chain_provider_id(
        &self,
        chain_provider_id: &str,
    ) -> Result<Option<Provider>, RepoError>;

    async fn list_by_verification_status(
        &self,
        status: VerificationStatus,
    ) -> Result<Vec<Provider>, RepoError>;
}

/// An account a provider has authorized to act on its behalf. Entirely
/// chain-sourced (besides `provider_id`, resolved by the indexer from the
/// event's account id) — see #15.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct ProviderStaff {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub stellar_account_id: String,
    pub role: StaffRole,
    pub status: StaffStatus,
    pub authorized_ledger: i64,
    pub removed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct ProviderStaffChainUpdate {
    pub provider_id: Uuid,
    pub stellar_account_id: String,
    pub role: StaffRole,
    pub status: StaffStatus,
    pub authorized_ledger: i64,
    pub removed_at: Option<OffsetDateTime>,
}

#[allow(async_fn_in_trait)]
pub trait ProviderStaffRepository {
    /// Upserts keyed on `(provider_id, stellar_account_id)` — deliberately
    /// not a global unique on the account, since one person can staff more
    /// than one provider. See `docs/schema.md`.
    async fn upsert_from_chain(
        &self,
        update: ProviderStaffChainUpdate,
    ) -> Result<ProviderStaff, RepoError>;

    async fn find_by_stellar_account_id(
        &self,
        provider_id: Uuid,
        stellar_account_id: &str,
    ) -> Result<Option<ProviderStaff>, RepoError>;

    async fn list_for_provider(&self, provider_id: Uuid) -> Result<Vec<ProviderStaff>, RepoError>;
}
