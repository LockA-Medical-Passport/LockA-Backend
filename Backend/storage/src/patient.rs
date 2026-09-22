use domain::{Patient, PatientChainUpdate, PatientRepository, PatientStatus, RepoError};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::map_sqlx_error;

/// `PatientRepository` backed by PostgreSQL.
pub struct PgPatientRepository {
    pool: PgPool,
}

impl PgPatientRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PatientRepository for PgPatientRepository {
    #[tracing::instrument(skip_all)]
    async fn upsert_from_chain(&self, update: PatientChainUpdate) -> Result<Patient, RepoError> {
        let id = Uuid::now_v7();

        // The `WHERE` guard on the conflict clause is what makes this safe
        // against out-of-order indexer replay: an event carrying an older
        // (or equal) `updated_ledger` than what's already stored is a
        // no-op, never a regression. When that guard rejects the write,
        // `RETURNING` produces no row — not an error, just "your write
        // didn't apply because a newer one already did" — so we fall back
        // to reading the row's current state instead of propagating
        // `RowNotFound`.
        let upserted = sqlx::query_as!(
            Patient,
            r#"
            INSERT INTO patients (
                id, stellar_account_id, passport_id, identity_commitment,
                recovery_config_hash, status, registered_ledger, registered_at, updated_ledger
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (stellar_account_id) DO UPDATE SET
                identity_commitment = EXCLUDED.identity_commitment,
                recovery_config_hash = EXCLUDED.recovery_config_hash,
                status = EXCLUDED.status,
                updated_ledger = EXCLUDED.updated_ledger
            WHERE EXCLUDED.updated_ledger > patients.updated_ledger
            RETURNING
                id, stellar_account_id, passport_id, identity_commitment,
                recovery_config_hash, status AS "status: PatientStatus",
                registered_ledger, registered_at, updated_ledger, first_indexed_at
            "#,
            id,
            update.stellar_account_id,
            update.passport_id,
            update.identity_commitment,
            update.recovery_config_hash,
            update.status as PatientStatus,
            update.registered_ledger,
            update.registered_at,
            update.updated_ledger,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        match upserted {
            Some(patient) => Ok(patient),
            None => self
                .find_by_stellar_account_id(&update.stellar_account_id)
                .await?
                .ok_or(RepoError::NotFound),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_stellar_account_id(
        &self,
        stellar_account_id: &str,
    ) -> Result<Option<Patient>, RepoError> {
        sqlx::query_as!(
            Patient,
            r#"
            SELECT
                id, stellar_account_id, passport_id, identity_commitment,
                recovery_config_hash, status AS "status: PatientStatus",
                registered_ledger, registered_at, updated_ledger, first_indexed_at
            FROM patients
            WHERE stellar_account_id = $1
            "#,
            stellar_account_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_passport_id(&self, passport_id: &str) -> Result<Option<Patient>, RepoError> {
        sqlx::query_as!(
            Patient,
            r#"
            SELECT
                id, stellar_account_id, passport_id, identity_commitment,
                recovery_config_hash, status AS "status: PatientStatus",
                registered_ledger, registered_at, updated_ledger, first_indexed_at
            FROM patients
            WHERE passport_id = $1
            "#,
            passport_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}

#[cfg(test)]
mod tests {
    use domain::PatientStatus;
    use time::OffsetDateTime;

    use super::*;

    fn sample_update(stellar_account_id: &str, updated_ledger: i64) -> PatientChainUpdate {
        PatientChainUpdate {
            stellar_account_id: stellar_account_id.to_string(),
            passport_id: format!("passport-{stellar_account_id}"),
            identity_commitment: vec![1, 2, 3],
            recovery_config_hash: None,
            status: PatientStatus::Active,
            registered_ledger: 100,
            registered_at: OffsetDateTime::now_utc(),
            updated_ledger,
        }
    }

    #[sqlx::test]
    async fn upsert_from_chain_inserts_a_new_patient(pool: PgPool) {
        let repo = PgPatientRepository::new(pool);

        let patient = repo
            .upsert_from_chain(sample_update(
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                1,
            ))
            .await
            .expect("insert should succeed");

        assert_eq!(patient.status, PatientStatus::Active);
        assert_eq!(patient.updated_ledger, 1);
    }

    #[sqlx::test]
    async fn upsert_from_chain_applies_a_newer_ledger(pool: PgPool) {
        let repo = PgPatientRepository::new(pool);
        let account = "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

        repo.upsert_from_chain(sample_update(account, 1)).await.expect("first insert");

        let mut newer = sample_update(account, 2);
        newer.status = PatientStatus::Suspended;
        let updated = repo.upsert_from_chain(newer).await.expect("newer update should apply");

        assert_eq!(updated.status, PatientStatus::Suspended);
        assert_eq!(updated.updated_ledger, 2);
    }

    #[sqlx::test]
    async fn upsert_from_chain_ignores_a_stale_ledger(pool: PgPool) {
        let repo = PgPatientRepository::new(pool);
        let account = "GCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

        repo.upsert_from_chain(sample_update(account, 5)).await.expect("first insert");

        let mut stale = sample_update(account, 3);
        stale.status = PatientStatus::Suspended;
        let result = repo.upsert_from_chain(stale).await.expect("stale update should not error");

        assert_eq!(result.status, PatientStatus::Active, "stale write must not regress state");
        assert_eq!(result.updated_ledger, 5);
    }

    #[sqlx::test]
    async fn find_by_passport_id_returns_none_when_missing(pool: PgPool) {
        let repo = PgPatientRepository::new(pool);

        let found = repo.find_by_passport_id("does-not-exist").await.expect("query should succeed");

        assert!(found.is_none());
    }
    #[sqlx::test]
    async fn equal_ledger_replay_cannot_overwrite_current_state(pool: PgPool) {
        let repo = PgPatientRepository::new(pool);
        let first = repo
            .upsert_from_chain(sample_update(
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                100,
            ))
            .await
            .unwrap();
        let mut replay = sample_update(&first.stellar_account_id, 100);
        replay.status = PatientStatus::Suspended;
        let result = repo.upsert_from_chain(replay).await.unwrap();
        assert_eq!(result, first);
        assert_eq!(repo.find_by_passport_id(&first.passport_id).await.unwrap(), Some(first));
    }
}
