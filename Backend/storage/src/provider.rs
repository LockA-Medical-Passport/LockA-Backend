use domain::{
    NewProvider, Provider, ProviderRepository, ProviderStaff, ProviderStaffChainUpdate,
    ProviderStaffRepository, ProviderType, RepoError, StaffRole, StaffStatus, VerificationStatus,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::map_sqlx_error;

/// `ProviderRepository` backed by PostgreSQL.
pub struct PgProviderRepository {
    pool: PgPool,
}

impl PgProviderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ProviderRepository for PgProviderRepository {
    #[tracing::instrument(skip_all)]
    async fn register(&self, new: NewProvider) -> Result<Provider, RepoError> {
        let id = Uuid::now_v7();

        sqlx::query_as!(
            Provider,
            r#"
            INSERT INTO providers (
                id, stellar_account_id, chain_provider_id, provider_type, verification_status,
                legal_name, display_name, country_code, contact_email, registered_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id, stellar_account_id, chain_provider_id,
                provider_type AS "provider_type: ProviderType",
                verification_status AS "verification_status: VerificationStatus",
                verified_ledger, legal_name, display_name, country_code, contact_email, registered_at
            "#,
            id,
            new.stellar_account_id,
            new.chain_provider_id,
            new.provider_type as ProviderType,
            new.verification_status as VerificationStatus,
            new.legal_name,
            new.display_name,
            new.country_code,
            new.contact_email,
            new.registered_at,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn update_verification_status_from_chain(
        &self,
        chain_provider_id: &str,
        verification_status: VerificationStatus,
        verified_ledger: i64,
    ) -> Result<Provider, RepoError> {
        // Same "guard against replaying an older event" pattern as
        // `PatientRepository::upsert_from_chain` — see its comment.
        let updated = sqlx::query_as!(
            Provider,
            r#"
            UPDATE providers
            SET verification_status = $2, verified_ledger = $3
            WHERE chain_provider_id = $1 AND $3 > COALESCE(verified_ledger, -1)
            RETURNING
                id, stellar_account_id, chain_provider_id,
                provider_type AS "provider_type: ProviderType",
                verification_status AS "verification_status: VerificationStatus",
                verified_ledger, legal_name, display_name, country_code, contact_email, registered_at
            "#,
            chain_provider_id,
            verification_status as VerificationStatus,
            verified_ledger,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        match updated {
            Some(provider) => Ok(provider),
            None => {
                self.find_by_chain_provider_id(chain_provider_id).await?.ok_or(RepoError::NotFound)
            }
        }
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_stellar_account_id(
        &self,
        stellar_account_id: &str,
    ) -> Result<Option<Provider>, RepoError> {
        sqlx::query_as!(
            Provider,
            r#"
            SELECT
                id, stellar_account_id, chain_provider_id,
                provider_type AS "provider_type: ProviderType",
                verification_status AS "verification_status: VerificationStatus",
                verified_ledger, legal_name, display_name, country_code, contact_email, registered_at
            FROM providers
            WHERE stellar_account_id = $1
            "#,
            stellar_account_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_chain_provider_id(
        &self,
        chain_provider_id: &str,
    ) -> Result<Option<Provider>, RepoError> {
        sqlx::query_as!(
            Provider,
            r#"
            SELECT
                id, stellar_account_id, chain_provider_id,
                provider_type AS "provider_type: ProviderType",
                verification_status AS "verification_status: VerificationStatus",
                verified_ledger, legal_name, display_name, country_code, contact_email, registered_at
            FROM providers
            WHERE chain_provider_id = $1
            "#,
            chain_provider_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn list_by_verification_status(
        &self,
        status: VerificationStatus,
    ) -> Result<Vec<Provider>, RepoError> {
        sqlx::query_as!(
            Provider,
            r#"
            SELECT
                id, stellar_account_id, chain_provider_id,
                provider_type AS "provider_type: ProviderType",
                verification_status AS "verification_status: VerificationStatus",
                verified_ledger, legal_name, display_name, country_code, contact_email, registered_at
            FROM providers
            WHERE verification_status = $1
            "#,
            status as VerificationStatus,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}

/// `ProviderStaffRepository` backed by PostgreSQL.
pub struct PgProviderStaffRepository {
    pool: PgPool,
}

impl PgProviderStaffRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ProviderStaffRepository for PgProviderStaffRepository {
    #[tracing::instrument(skip_all)]
    async fn upsert_from_chain(
        &self,
        update: ProviderStaffChainUpdate,
    ) -> Result<ProviderStaff, RepoError> {
        let id = Uuid::now_v7();

        let upserted = sqlx::query_as!(
            ProviderStaff,
            r#"
            INSERT INTO provider_staff (
                id, provider_id, stellar_account_id, role, status, authorized_ledger, removed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (provider_id, stellar_account_id) DO UPDATE SET
                role = EXCLUDED.role,
                status = EXCLUDED.status,
                authorized_ledger = EXCLUDED.authorized_ledger,
                removed_at = EXCLUDED.removed_at
            WHERE EXCLUDED.authorized_ledger > provider_staff.authorized_ledger
            RETURNING
                id, provider_id, stellar_account_id,
                role AS "role: StaffRole", status AS "status: StaffStatus",
                authorized_ledger, removed_at
            "#,
            id,
            update.provider_id,
            update.stellar_account_id,
            update.role as StaffRole,
            update.status as StaffStatus,
            update.authorized_ledger,
            update.removed_at,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        match upserted {
            Some(staff) => Ok(staff),
            None => self
                .find_by_stellar_account_id(update.provider_id, &update.stellar_account_id)
                .await?
                .ok_or(RepoError::NotFound),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_stellar_account_id(
        &self,
        provider_id: Uuid,
        stellar_account_id: &str,
    ) -> Result<Option<ProviderStaff>, RepoError> {
        sqlx::query_as!(
            ProviderStaff,
            r#"
            SELECT
                id, provider_id, stellar_account_id,
                role AS "role: StaffRole", status AS "status: StaffStatus",
                authorized_ledger, removed_at
            FROM provider_staff
            WHERE provider_id = $1 AND stellar_account_id = $2
            "#,
            provider_id,
            stellar_account_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn list_for_provider(&self, provider_id: Uuid) -> Result<Vec<ProviderStaff>, RepoError> {
        sqlx::query_as!(
            ProviderStaff,
            r#"
            SELECT
                id, provider_id, stellar_account_id,
                role AS "role: StaffRole", status AS "status: StaffStatus",
                authorized_ledger, removed_at
            FROM provider_staff
            WHERE provider_id = $1
            "#,
            provider_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    async fn seed_provider(
        pool: &PgPool,
        stellar_account_id: &str,
        chain_provider_id: &str,
    ) -> Uuid {
        let repo = PgProviderRepository::new(pool.clone());
        let provider = repo
            .register(NewProvider {
                stellar_account_id: stellar_account_id.to_string(),
                chain_provider_id: chain_provider_id.to_string(),
                provider_type: ProviderType::Clinic,
                verification_status: VerificationStatus::Pending,
                legal_name: "Test Clinic".to_string(),
                display_name: None,
                country_code: None,
                contact_email: None,
                registered_at: OffsetDateTime::now_utc(),
            })
            .await
            .expect("register should succeed");
        provider.id
    }

    #[sqlx::test]
    async fn register_then_find_round_trips(pool: PgPool) {
        let provider_id = seed_provider(
            &pool,
            "GDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
            "chain-1",
        )
        .await;

        let repo = PgProviderRepository::new(pool);
        let found = repo
            .find_by_chain_provider_id("chain-1")
            .await
            .expect("query should succeed")
            .expect("should exist");

        assert_eq!(found.id, provider_id);
        assert_eq!(found.verification_status, VerificationStatus::Pending);
    }

    #[sqlx::test]
    async fn registering_a_duplicate_chain_provider_id_conflicts(pool: PgPool) {
        seed_provider(
            &pool,
            "GEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE",
            "chain-dup",
        )
        .await;

        let repo = PgProviderRepository::new(pool);
        let result = repo
            .register(NewProvider {
                stellar_account_id: "GFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
                    .to_string(),
                chain_provider_id: "chain-dup".to_string(),
                provider_type: ProviderType::Hospital,
                verification_status: VerificationStatus::Pending,
                legal_name: "Another Hospital".to_string(),
                display_name: None,
                country_code: None,
                contact_email: None,
                registered_at: OffsetDateTime::now_utc(),
            })
            .await;

        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[sqlx::test]
    async fn update_verification_status_from_chain_updates_an_existing_provider(pool: PgPool) {
        seed_provider(
            &pool,
            "GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG",
            "chain-verify",
        )
        .await;

        let repo = PgProviderRepository::new(pool);
        let updated = repo
            .update_verification_status_from_chain("chain-verify", VerificationStatus::Verified, 42)
            .await
            .expect("update should succeed");

        assert_eq!(updated.verification_status, VerificationStatus::Verified);
        assert_eq!(updated.verified_ledger, Some(42));
    }

    #[sqlx::test]
    async fn provider_staff_upsert_is_scoped_per_provider(pool: PgPool) {
        let provider_a = seed_provider(
            &pool,
            "GHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHH",
            "chain-a",
        )
        .await;
        let provider_b = seed_provider(
            &pool,
            "GIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII",
            "chain-b",
        )
        .await;
        let staff_account = "GJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJ";

        let repo = PgProviderStaffRepository::new(pool);
        repo.upsert_from_chain(ProviderStaffChainUpdate {
            provider_id: provider_a,
            stellar_account_id: staff_account.to_string(),
            role: StaffRole::Clinician,
            status: StaffStatus::Active,
            authorized_ledger: 1,
            removed_at: None,
        })
        .await
        .expect("first authorization should succeed");

        // The same account, authorized by a *different* provider, must be
        // representable as a second row (locum work) rather than treated
        // as a conflicting duplicate.
        repo.upsert_from_chain(ProviderStaffChainUpdate {
            provider_id: provider_b,
            stellar_account_id: staff_account.to_string(),
            role: StaffRole::Technician,
            status: StaffStatus::Active,
            authorized_ledger: 1,
            removed_at: None,
        })
        .await
        .expect("second authorization at a different provider should succeed");

        let at_a = repo
            .find_by_stellar_account_id(provider_a, staff_account)
            .await
            .expect("query should succeed")
            .expect("should exist");
        assert_eq!(at_a.role, StaffRole::Clinician);

        let at_b = repo
            .find_by_stellar_account_id(provider_b, staff_account)
            .await
            .expect("query should succeed")
            .expect("should exist");
        assert_eq!(at_b.role, StaffRole::Technician);
    }
}
