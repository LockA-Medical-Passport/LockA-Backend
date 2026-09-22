use crate::RepoError;
use time::OffsetDateTime;

/// Persistent SEP-10 replay protection, shared across processes and restarts.
#[async_trait::async_trait]
pub trait ChallengeRepository: Send + Sync {
    async fn insert(
        &self,
        hash: &[u8; 32],
        account: &str,
        expires_at: OffsetDateTime,
    ) -> Result<(), RepoError>;
    /// Atomically consume an issued, unexpired challenge once. False means
    /// unknown, expired, or previously consumed; concurrent requests cannot both win.
    async fn consume(&self, hash: &[u8; 32], account: &str) -> Result<bool, RepoError>;
    async fn delete_expired(&self) -> Result<u64, RepoError>;
}
