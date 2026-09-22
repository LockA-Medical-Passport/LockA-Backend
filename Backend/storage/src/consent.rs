use domain::{
    AccessRequest, AccessRequestChainUpdate, AccessRequestStatus, ConsentGrant,
    ConsentGrantChainUpdate, ConsentRepository, PurposeCode, RecordCategory, RepoError,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::map_sqlx_error;

/// `ConsentRepository` backed by PostgreSQL, covering both halves of
/// `ConsentAccessControl`'s read model: `access_requests` and
/// `consent_grants`.
pub struct PgConsentRepository {
    pool: PgPool,
}

impl PgConsentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ConsentRepository for PgConsentRepository {
    #[tracing::instrument(skip_all)]
    async fn upsert_access_request_from_chain(
        &self,
        update: AccessRequestChainUpdate,
    ) -> Result<AccessRequest, RepoError> {
        let id = Uuid::now_v7();

        // `patient_id`/`provider_id`/`requested_by_staff_id`/`record_category`/
        // `purpose_code`/`requested_duration_secs`/`requested_ledger`/`requested_at`
        // are set once at creation and never change. Only the resolution
        // (`status`, `resolved_ledger`, `resolved_at`) can arrive later as a
        // second event for the same `chain_request_id` — so a replayed
        // *creation* event (which always carries `resolved_ledger: None`)
        // must not clobber a resolution that already landed, and a replayed
        // *resolution* event must not regress past a newer one.
        sqlx::query_as!(
            AccessRequest,
            r#"
            INSERT INTO access_requests (
                id, chain_request_id, patient_id, provider_id, requested_by_staff_id,
                record_category, purpose_code, requested_duration_secs, status,
                requested_ledger, requested_at, resolved_ledger, resolved_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (chain_request_id) DO UPDATE SET
                status = CASE
                    WHEN EXCLUDED.resolved_ledger IS NULL THEN access_requests.status
                    WHEN EXCLUDED.resolved_ledger > COALESCE(access_requests.resolved_ledger, -1)
                        THEN EXCLUDED.status
                    ELSE access_requests.status
                END,
                resolved_ledger = CASE
                    WHEN EXCLUDED.resolved_ledger IS NULL THEN access_requests.resolved_ledger
                    WHEN EXCLUDED.resolved_ledger > COALESCE(access_requests.resolved_ledger, -1)
                        THEN EXCLUDED.resolved_ledger
                    ELSE access_requests.resolved_ledger
                END,
                resolved_at = CASE
                    WHEN EXCLUDED.resolved_ledger IS NULL THEN access_requests.resolved_at
                    WHEN EXCLUDED.resolved_ledger > COALESCE(access_requests.resolved_ledger, -1)
                        THEN EXCLUDED.resolved_at
                    ELSE access_requests.resolved_at
                END
            RETURNING
                id, chain_request_id, patient_id, provider_id, requested_by_staff_id,
                record_category AS "record_category: RecordCategory",
                purpose_code AS "purpose_code: PurposeCode",
                requested_duration_secs, status AS "status: AccessRequestStatus",
                requested_ledger, requested_at, resolved_ledger, resolved_at
            "#,
            id,
            update.chain_request_id,
            update.patient_id,
            update.provider_id,
            update.requested_by_staff_id,
            update.record_category as RecordCategory,
            update.purpose_code as PurposeCode,
            update.requested_duration_secs,
            update.status as AccessRequestStatus,
            update.requested_ledger,
            update.requested_at,
            update.resolved_ledger,
            update.resolved_at,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn upsert_consent_grant_from_chain(
        &self,
        update: ConsentGrantChainUpdate,
    ) -> Result<ConsentGrant, RepoError> {
        let id = Uuid::now_v7();

        // Same replay-safety reasoning as the access-request upsert above:
        // only `revoked_at`/`revoked_ledger` can arrive as a later event.
        sqlx::query_as!(
            ConsentGrant,
            r#"
            INSERT INTO consent_grants (
                id, chain_grant_id, access_request_id, patient_id, provider_id,
                record_category, granted_ledger, granted_at, expires_at,
                revoked_at, revoked_ledger
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (chain_grant_id) DO UPDATE SET
                revoked_at = CASE
                    WHEN EXCLUDED.revoked_ledger IS NULL THEN consent_grants.revoked_at
                    WHEN EXCLUDED.revoked_ledger > COALESCE(consent_grants.revoked_ledger, -1)
                        THEN EXCLUDED.revoked_at
                    ELSE consent_grants.revoked_at
                END,
                revoked_ledger = CASE
                    WHEN EXCLUDED.revoked_ledger IS NULL THEN consent_grants.revoked_ledger
                    WHEN EXCLUDED.revoked_ledger > COALESCE(consent_grants.revoked_ledger, -1)
                        THEN EXCLUDED.revoked_ledger
                    ELSE consent_grants.revoked_ledger
                END
            RETURNING
                id, chain_grant_id, access_request_id, patient_id, provider_id,
                record_category AS "record_category: RecordCategory",
                granted_ledger, granted_at, expires_at, revoked_at, revoked_ledger
            "#,
            id,
            update.chain_grant_id,
            update.access_request_id,
            update.patient_id,
            update.provider_id,
            update.record_category as RecordCategory,
            update.granted_ledger,
            update.granted_at,
            update.expires_at,
            update.revoked_at,
            update.revoked_ledger,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn list_access_requests_for_patient(
        &self,
        patient_id: Uuid,
        status: Option<AccessRequestStatus>,
    ) -> Result<Vec<AccessRequest>, RepoError> {
        sqlx::query_as!(
            AccessRequest,
            r#"
            SELECT
                id, chain_request_id, patient_id, provider_id, requested_by_staff_id,
                record_category AS "record_category: RecordCategory",
                purpose_code AS "purpose_code: PurposeCode",
                requested_duration_secs, status AS "status: AccessRequestStatus",
                requested_ledger, requested_at, resolved_ledger, resolved_at
            FROM access_requests
            WHERE patient_id = $1
              AND ($2::access_request_status IS NULL OR status = $2)
            ORDER BY requested_at DESC
            "#,
            patient_id,
            status as Option<AccessRequestStatus>,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn active_grant(
        &self,
        patient_id: Uuid,
        provider_id: Uuid,
        category: RecordCategory,
    ) -> Result<Option<ConsentGrant>, RepoError> {
        sqlx::query_as!(
            ConsentGrant,
            r#"
            SELECT
                id, chain_grant_id, access_request_id, patient_id, provider_id,
                record_category AS "record_category: RecordCategory",
                granted_ledger, granted_at, expires_at, revoked_at, revoked_ledger
            FROM consent_grants
            WHERE patient_id = $1
              AND provider_id = $2
              AND record_category = $3
              AND revoked_at IS NULL
              AND expires_at > now()
            ORDER BY granted_at DESC
            LIMIT 1
            "#,
            patient_id,
            provider_id,
            category as RecordCategory,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}

#[cfg(test)]
mod tests {
    use domain::{PatientRepository, ProviderRepository};
    use time::{Duration, OffsetDateTime};

    use super::*;
    use crate::patient::PgPatientRepository;
    use crate::provider::PgProviderRepository;

    async fn seed_patient(pool: &PgPool, stellar_account_id: &str) -> Uuid {
        let repo = PgPatientRepository::new(pool.clone());
        let patient = repo
            .upsert_from_chain(domain::PatientChainUpdate {
                stellar_account_id: stellar_account_id.to_string(),
                passport_id: format!("passport-{stellar_account_id}"),
                identity_commitment: vec![9],
                recovery_config_hash: None,
                status: domain::PatientStatus::Active,
                registered_ledger: 1,
                registered_at: OffsetDateTime::now_utc(),
                updated_ledger: 1,
            })
            .await
            .expect("seed patient");
        patient.id
    }

    async fn seed_provider(
        pool: &PgPool,
        stellar_account_id: &str,
        chain_provider_id: &str,
    ) -> Uuid {
        let repo = PgProviderRepository::new(pool.clone());
        let provider = repo
            .register(domain::NewProvider {
                stellar_account_id: stellar_account_id.to_string(),
                chain_provider_id: chain_provider_id.to_string(),
                provider_type: domain::ProviderType::Clinic,
                verification_status: domain::VerificationStatus::Verified,
                legal_name: "Test Clinic".to_string(),
                display_name: None,
                country_code: None,
                contact_email: None,
                registered_at: OffsetDateTime::now_utc(),
            })
            .await
            .expect("seed provider");
        provider.id
    }

    #[sqlx::test]
    async fn access_request_resolution_does_not_get_clobbered_by_a_replayed_creation(pool: PgPool) {
        let patient_id =
            seed_patient(&pool, "GKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKK").await;
        let provider_id = seed_provider(
            &pool,
            "GLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLL",
            "chain-p1",
        )
        .await;

        let repo = PgConsentRepository::new(pool);
        let base = AccessRequestChainUpdate {
            chain_request_id: "chain-req-1".to_string(),
            patient_id,
            provider_id,
            requested_by_staff_id: None,
            record_category: RecordCategory::Laboratory,
            purpose_code: PurposeCode::Treatment,
            requested_duration_secs: 3600,
            status: AccessRequestStatus::Pending,
            requested_ledger: 10,
            requested_at: OffsetDateTime::now_utc(),
            resolved_ledger: None,
            resolved_at: None,
        };

        repo.upsert_access_request_from_chain(base.clone()).await.expect("initial creation");

        let mut resolved = base.clone();
        resolved.status = AccessRequestStatus::Approved;
        resolved.resolved_ledger = Some(20);
        resolved.resolved_at = Some(OffsetDateTime::now_utc());
        repo.upsert_access_request_from_chain(resolved).await.expect("resolution");

        // The indexer replays the original creation event again (e.g. after
        // a restart, re-scanning from an earlier checkpoint).
        let replayed =
            repo.upsert_access_request_from_chain(base).await.expect("replayed creation");

        assert_eq!(
            replayed.status,
            AccessRequestStatus::Approved,
            "a replayed creation event must not revert an already-resolved request"
        );
        assert_eq!(replayed.resolved_ledger, Some(20));
    }

    #[sqlx::test]
    async fn active_grant_excludes_revoked_and_expired_grants(pool: PgPool) {
        let patient_id =
            seed_patient(&pool, "GMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM").await;
        let provider_id = seed_provider(
            &pool,
            "GNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNN",
            "chain-p2",
        )
        .await;

        let repo = PgConsentRepository::new(pool);
        let now = OffsetDateTime::now_utc();

        // Expired grant: must not count as active.
        repo.upsert_consent_grant_from_chain(ConsentGrantChainUpdate {
            chain_grant_id: "grant-expired".to_string(),
            access_request_id: None,
            patient_id,
            provider_id,
            record_category: RecordCategory::Laboratory,
            granted_ledger: 1,
            granted_at: now - Duration::hours(2),
            expires_at: now - Duration::hours(1),
            revoked_at: None,
            revoked_ledger: None,
        })
        .await
        .expect("expired grant insert");

        assert!(
            repo.active_grant(patient_id, provider_id, RecordCategory::Laboratory)
                .await
                .expect("query should succeed")
                .is_none(),
            "an expired grant must not be reported as active"
        );

        // A currently-valid grant.
        repo.upsert_consent_grant_from_chain(ConsentGrantChainUpdate {
            chain_grant_id: "grant-active".to_string(),
            access_request_id: None,
            patient_id,
            provider_id,
            record_category: RecordCategory::Laboratory,
            granted_ledger: 2,
            granted_at: now,
            expires_at: now + Duration::hours(1),
            revoked_at: None,
            revoked_ledger: None,
        })
        .await
        .expect("active grant insert");

        let active = repo
            .active_grant(patient_id, provider_id, RecordCategory::Laboratory)
            .await
            .expect("query should succeed")
            .expect("should find the active grant");
        assert_eq!(active.chain_grant_id, "grant-active");

        // Revoking it must remove it from the active check.
        repo.upsert_consent_grant_from_chain(ConsentGrantChainUpdate {
            chain_grant_id: "grant-active".to_string(),
            access_request_id: None,
            patient_id,
            provider_id,
            record_category: RecordCategory::Laboratory,
            granted_ledger: 2,
            granted_at: now,
            expires_at: now + Duration::hours(1),
            revoked_at: Some(now),
            revoked_ledger: Some(3),
        })
        .await
        .expect("revocation");

        assert!(
            repo.active_grant(patient_id, provider_id, RecordCategory::Laboratory)
                .await
                .expect("query should succeed")
                .is_none(),
            "a revoked grant must not be reported as active"
        );
    }
}
