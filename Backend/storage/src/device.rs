use domain::{
    DeviceReadingIndexEntry, DeviceRegistration, DeviceRegistrationChainUpdate, DeviceRepository,
    DeviceStatus, DeviceType, NewDeviceReadingIndexEntry, RepoError, StorageBackend,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::map_sqlx_error;

/// `DeviceRepository` backed by PostgreSQL, covering `device_registrations`
/// and `device_readings_index`.
pub struct PgDeviceRepository {
    pool: PgPool,
}

impl PgDeviceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl DeviceRepository for PgDeviceRepository {
    #[tracing::instrument(skip_all)]
    async fn upsert_registration_from_chain(
        &self,
        update: DeviceRegistrationChainUpdate,
    ) -> Result<DeviceRegistration, RepoError> {
        let id = Uuid::now_v7();

        // Unlike `consent_grants`, this table has no separate
        // `revoked_ledger` to compare against — `registered_ledger` is the
        // creation ledger and never changes. So instead of a ledger-number
        // guard, this ratchets on `status`: once a row is `revoked`, no
        // replayed event (which `DeviceAttestationRegistry` never emits as
        // an "un-revoke" anyway) can move it back to `active`.
        sqlx::query_as!(
            DeviceRegistration,
            r#"
            INSERT INTO device_registrations (
                id, chain_device_id, patient_id, device_public_key, device_type, status,
                registered_ledger, registered_at, revoked_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (chain_device_id) DO UPDATE SET
                status = CASE
                    WHEN device_registrations.status = 'revoked' THEN device_registrations.status
                    ELSE EXCLUDED.status
                END,
                revoked_at = CASE
                    WHEN device_registrations.status = 'revoked' THEN device_registrations.revoked_at
                    ELSE EXCLUDED.revoked_at
                END
            RETURNING
                id, chain_device_id, patient_id, device_public_key,
                device_type AS "device_type: DeviceType", status AS "status: DeviceStatus",
                registered_ledger, registered_at, revoked_at
            "#,
            id,
            update.chain_device_id,
            update.patient_id,
            update.device_public_key,
            update.device_type as DeviceType,
            update.status as DeviceStatus,
            update.registered_ledger,
            update.registered_at,
            update.revoked_at,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn find_registration_by_chain_device_id(
        &self,
        chain_device_id: &str,
    ) -> Result<Option<DeviceRegistration>, RepoError> {
        sqlx::query_as!(
            DeviceRegistration,
            r#"
            SELECT
                id, chain_device_id, patient_id, device_public_key,
                device_type AS "device_type: DeviceType", status AS "status: DeviceStatus",
                registered_ledger, registered_at, revoked_at
            FROM device_registrations
            WHERE chain_device_id = $1
            "#,
            chain_device_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn insert_reading(
        &self,
        new: NewDeviceReadingIndexEntry,
    ) -> Result<DeviceReadingIndexEntry, RepoError> {
        let id = Uuid::now_v7();

        sqlx::query_as!(
            DeviceReadingIndexEntry,
            r#"
            INSERT INTO device_readings_index (
                id, device_registration_id, patient_id, recorded_at, signature_verified,
                ciphertext_sha256, storage_backend, storage_uri, encrypted_data_key,
                key_encryption_key_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id, device_registration_id, patient_id, record_index_id, recorded_at,
                ingested_at, signature_verified, ciphertext_sha256,
                storage_backend AS "storage_backend: StorageBackend",
                storage_uri, encrypted_data_key, key_encryption_key_id
            "#,
            id,
            new.device_registration_id,
            new.patient_id,
            new.recorded_at,
            new.signature_verified,
            new.ciphertext_sha256,
            new.storage_backend as StorageBackend,
            new.storage_uri,
            new.encrypted_data_key,
            new.key_encryption_key_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    #[tracing::instrument(skip_all)]
    async fn list_readings_for_patient(
        &self,
        patient_id: Uuid,
    ) -> Result<Vec<DeviceReadingIndexEntry>, RepoError> {
        sqlx::query_as!(
            DeviceReadingIndexEntry,
            r#"
            SELECT
                id, device_registration_id, patient_id, record_index_id, recorded_at,
                ingested_at, signature_verified, ciphertext_sha256,
                storage_backend AS "storage_backend: StorageBackend",
                storage_uri, encrypted_data_key, key_encryption_key_id
            FROM device_readings_index
            WHERE patient_id = $1
            ORDER BY recorded_at DESC
            "#,
            patient_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}

#[cfg(test)]
mod tests {
    use domain::PatientRepository;
    use time::OffsetDateTime;

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

    #[sqlx::test]
    async fn revocation_cannot_be_undone_by_a_replayed_registration_event(pool: PgPool) {
        let patient_id =
            seed_patient(&pool, "GRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRR").await;
        let repo = PgDeviceRepository::new(pool);

        let registration_event = DeviceRegistrationChainUpdate {
            chain_device_id: "device-1".to_string(),
            patient_id,
            device_public_key: vec![1; 32],
            device_type: DeviceType::Glucometer,
            status: DeviceStatus::Active,
            registered_ledger: 1,
            registered_at: OffsetDateTime::now_utc(),
            revoked_at: None,
        };
        repo.upsert_registration_from_chain(registration_event.clone())
            .await
            .expect("initial registration");

        let mut revoked_event = registration_event.clone();
        revoked_event.status = DeviceStatus::Revoked;
        revoked_event.revoked_at = Some(OffsetDateTime::now_utc());
        repo.upsert_registration_from_chain(revoked_event).await.expect("revocation");

        // The indexer replays the original registration event.
        let replayed = repo
            .upsert_registration_from_chain(registration_event)
            .await
            .expect("replayed registration should not error");

        assert_eq!(
            replayed.status,
            DeviceStatus::Revoked,
            "a replayed registration event must not un-revoke a device"
        );
    }

    #[sqlx::test]
    async fn duplicate_reading_ciphertext_conflicts(pool: PgPool) {
        let patient_id =
            seed_patient(&pool, "GSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS").await;
        let device_repo = PgDeviceRepository::new(pool.clone());
        let registration = device_repo
            .upsert_registration_from_chain(DeviceRegistrationChainUpdate {
                chain_device_id: "device-2".to_string(),
                patient_id,
                device_public_key: vec![2; 32],
                device_type: DeviceType::Thermometer,
                status: DeviceStatus::Active,
                registered_ledger: 1,
                registered_at: OffsetDateTime::now_utc(),
                revoked_at: None,
            })
            .await
            .expect("registration");

        let reading = NewDeviceReadingIndexEntry {
            device_registration_id: registration.id,
            patient_id,
            recorded_at: OffsetDateTime::now_utc(),
            signature_verified: true,
            ciphertext_sha256: vec![5; 32],
            storage_backend: StorageBackend::S3,
            storage_uri: "s3://bucket/reading".to_string(),
            encrypted_data_key: vec![1, 2, 3],
            key_encryption_key_id: "kek-1".to_string(),
        };

        device_repo.insert_reading(reading.clone()).await.expect("first reading should succeed");
        let result = device_repo.insert_reading(reading).await;

        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }
}
