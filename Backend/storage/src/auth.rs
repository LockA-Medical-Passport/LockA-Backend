use crate::error::map_sqlx_error;
use domain::{ChallengeRepository, RepoError};
use sqlx::PgPool;
use time::OffsetDateTime;

pub struct PgChallengeRepository {
    pool: PgPool,
}
impl PgChallengeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ChallengeRepository for PgChallengeRepository {
    #[tracing::instrument(skip_all)]
    async fn insert(
        &self,
        hash: &[u8; 32],
        account: &str,
        expires_at: OffsetDateTime,
    ) -> Result<(), RepoError> {
        sqlx::query!("INSERT INTO auth_challenges (transaction_hash, stellar_account_id, expires_at) VALUES ($1, $2, $3)",
            &hash[..], account, expires_at).execute(&self.pool).await.map_err(map_sqlx_error)?;
        Ok(())
    }
    #[tracing::instrument(skip_all)]
    async fn consume(&self, hash: &[u8; 32], account: &str) -> Result<bool, RepoError> {
        let result = sqlx::query!("UPDATE auth_challenges SET consumed_at = now() WHERE transaction_hash = $1 AND stellar_account_id = $2 AND consumed_at IS NULL AND expires_at > now()",
            &hash[..], account).execute(&self.pool).await.map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }
    #[tracing::instrument(skip_all)]
    async fn delete_expired(&self) -> Result<u64, RepoError> {
        let result = sqlx::query!("DELETE FROM auth_challenges WHERE expires_at <= now()")
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    #[sqlx::test]
    async fn challenge_consumption_is_atomic_across_repository_instances(pool: PgPool) {
        let first = PgChallengeRepository::new(pool.clone());
        let second = PgChallengeRepository::new(pool);
        first
            .insert(&[1; 32], ACCOUNT, OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .await
            .unwrap();
        let (a, b) =
            tokio::join!(first.consume(&[1; 32], ACCOUNT), second.consume(&[1; 32], ACCOUNT));
        assert_ne!(a.unwrap(), b.unwrap());
        assert!(!second.consume(&[1; 32], ACCOUNT).await.unwrap());
    }
    #[sqlx::test]
    async fn unknown_expired_and_wrong_account_challenges_cannot_be_consumed(pool: PgPool) {
        let repo = PgChallengeRepository::new(pool);
        assert!(!repo.consume(&[1; 32], ACCOUNT).await.unwrap());
        repo.insert(&[1; 32], ACCOUNT, OffsetDateTime::now_utc() - time::Duration::seconds(1))
            .await
            .unwrap();
        assert!(!repo.consume(&[1; 32], ACCOUNT).await.unwrap());
        repo.insert(&[2; 32], ACCOUNT, OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .await
            .unwrap();
        assert!(
            !repo
                .consume(&[2; 32], "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")
                .await
                .unwrap()
        );
        assert_eq!(repo.delete_expired().await.unwrap(), 1);
        assert!(repo.consume(&[2; 32], ACCOUNT).await.unwrap());
    }
}
