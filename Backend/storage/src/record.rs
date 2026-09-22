use domain::{
    NewRecordIndexEntry, RecordCategory, RecordIndexEntry, RecordIndexRepository, RepoError,
    StorageBackend,
};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::map_sqlx_error;

/// `RecordIndexRepository` backed by PostgreSQL.
pub struct PgRecordIndexRepository {
    pool: PgPool,
}

impl PgRecordIndexRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl RecordIndexRepository for PgRecordIndexRepository {
    #[tracing::instrument(skip_all)]
    async fn insert(&self, new: NewRecordIndexEntry) -> Result<RecordIndexEntry, RepoError> {
        let id = Uuid::now_v7();

        sqlx::query_as!(
            RecordIndexEntry,
            r#"
            INSERT INTO record_index (
                id, patient_id, ciphertext_sha256, record_category, issuer_provider_id,
                issued_by_staff_id, storage_backend, storage_uri, ciphertext_size_bytes,
                encrypted_data_key, key_encryption_key_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING
                id, patient_id, ciphertext_sha256,
                record_category AS "record_category: RecordCategory",
                issuer_provider_id, issued_by_staff_id,
                storage_backend AS "storage_backend: StorageBackend",
                storage_uri, ciphertext_size_bytes, encrypted_data_key, key_encryption_key_id,
                anchored_ledger, anchored_at, superseded_by_id, created_at
            "#,
            id,
            new.patient_id,
            new.ciphertext_sha256,
            new.record_category as RecordCategory,
            new.issuer_provider_id,
            new.issued_by_staff_id,
            new.storage_backend as StorageBackend,
            new.storage_uri,
            new.ciphertext_size_bytes,
            new.encrypted_data_key,
            new.key_encryption_key_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn mark_anchored(
        &self,
        id: Uuid,
        anchored_ledger: i64,
        anchored_at: OffsetDateTime,
    ) -> Result<RecordIndexEntry, RepoError> {
        let updated = sqlx::query_as!(
            RecordIndexEntry,
            r#"
            UPDATE record_index
            SET anchored_ledger = $2, anchored_at = $3
            WHERE id = $1 AND $2 > COALESCE(anchored_ledger, -1)
            RETURNING
                id, patient_id, ciphertext_sha256,
                record_category AS "record_category: RecordCategory",
                issuer_provider_id, issued_by_staff_id,
                storage_backend AS "storage_backend: StorageBackend",
                storage_uri, ciphertext_size_bytes, encrypted_data_key, key_encryption_key_id,
                anchored_ledger, anchored_at, superseded_by_id, created_at
            "#,
            id,
            anchored_ledger,
            anchored_at,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        match updated {
            Some(entry) => Ok(entry),
            None => sqlx::query_as!(
                RecordIndexEntry,
                r#"
                SELECT
                    id, patient_id, ciphertext_sha256,
                    record_category AS "record_category: RecordCategory",
                    issuer_provider_id, issued_by_staff_id,
                    storage_backend AS "storage_backend: StorageBackend",
                    storage_uri, ciphertext_size_bytes, encrypted_data_key, key_encryption_key_id,
                    anchored_ledger, anchored_at, superseded_by_id, created_at
                FROM record_index
                WHERE id = $1
                "#,
                id,
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .ok_or(RepoError::NotFound),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn list_for_patient(
        &self,
        patient_id: Uuid,
        category: Option<RecordCategory>,
    ) -> Result<Vec<RecordIndexEntry>, RepoError> {
        sqlx::query_as!(
            RecordIndexEntry,
            r#"
            SELECT
                id, patient_id, ciphertext_sha256,
                record_category AS "record_category: RecordCategory",
                issuer_provider_id, issued_by_staff_id,
                storage_backend AS "storage_backend: StorageBackend",
                storage_uri, ciphertext_size_bytes, encrypted_data_key, key_encryption_key_id,
                anchored_ledger, anchored_at, superseded_by_id, created_at
            FROM record_index
            WHERE patient_id = $1
              AND ($2::record_category IS NULL OR record_category = $2)
            ORDER BY created_at DESC
            "#,
            patient_id,
            category as Option<RecordCategory>,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_ciphertext_hash(
        &self,
        ciphertext_sha256: &[u8],
    ) -> Result<Option<RecordIndexEntry>, RepoError> {
        sqlx::query_as!(
            RecordIndexEntry,
            r#"
            SELECT
                id, patient_id, ciphertext_sha256,
                record_category AS "record_category: RecordCategory",
                issuer_provider_id, issued_by_staff_id,
                storage_backend AS "storage_backend: StorageBackend",
                storage_uri, ciphertext_size_bytes, encrypted_data_key, key_encryption_key_id,
                anchored_ledger, anchored_at, superseded_by_id, created_at
            FROM record_index
            WHERE ciphertext_sha256 = $1
            "#,
            ciphertext_sha256,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}

#[cfg(test)]
mod tests {
    use domain::PatientRepository;

    use super::*;
    use crate::patient::PgPatientRepository;

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

    fn sample_entry(patient_id: Uuid, hash: u8) -> NewRecordIndexEntry {
        NewRecordIndexEntry {
            patient_id,
            ciphertext_sha256: vec![hash; 32],
            record_category: RecordCategory::Laboratory,
            issuer_provider_id: None,
            issued_by_staff_id: None,
            storage_backend: StorageBackend::S3,
            storage_uri: "s3://bucket/object".to_string(),
            ciphertext_size_bytes: 1024,
            encrypted_data_key: vec![1, 2, 3],
            key_encryption_key_id: "kek-1".to_string(),
        }
    }

    #[sqlx::test]
    async fn insert_then_mark_anchored(pool: PgPool) {
        let patient_id =
            seed_patient(&pool, "GOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO").await;
        let repo = PgRecordIndexRepository::new(pool);

        let entry = repo.insert(sample_entry(patient_id, 1)).await.expect("insert should succeed");
        assert!(entry.anchored_ledger.is_none());

        let anchored = repo
            .mark_anchored(entry.id, 100, OffsetDateTime::now_utc())
            .await
            .expect("mark_anchored should succeed");
        assert_eq!(anchored.anchored_ledger, Some(100));
    }

    #[sqlx::test]
    async fn duplicate_ciphertext_hash_conflicts(pool: PgPool) {
        let patient_id =
            seed_patient(&pool, "GPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP").await;
        let repo = PgRecordIndexRepository::new(pool);

        repo.insert(sample_entry(patient_id, 7)).await.expect("first insert should succeed");
        let result = repo.insert(sample_entry(patient_id, 7)).await;

        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[sqlx::test]
    async fn list_for_patient_filters_by_category(pool: PgPool) {
        let patient_id =
            seed_patient(&pool, "GQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ").await;
        let repo = PgRecordIndexRepository::new(pool);

        repo.insert(sample_entry(patient_id, 2)).await.expect("insert laboratory record");
        let mut imaging = sample_entry(patient_id, 3);
        imaging.record_category = RecordCategory::Imaging;
        repo.insert(imaging).await.expect("insert imaging record");

        let laboratory_only = repo
            .list_for_patient(patient_id, Some(RecordCategory::Laboratory))
            .await
            .expect("query should succeed");
        assert_eq!(laboratory_only.len(), 1);
        assert_eq!(laboratory_only[0].record_category, RecordCategory::Laboratory);

        let all = repo.list_for_patient(patient_id, None).await.expect("query should succeed");
        assert_eq!(all.len(), 2);
    }
}
