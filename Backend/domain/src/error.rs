//! The error type every repository method returns. `storage`'s Postgres
//! implementations map `sqlx::Error` into one of these variants; nothing in
//! `domain`'s public API names `sqlx::Error` (or any other backend-specific
//! error type) directly.

/// An error returned by a repository method.
#[derive(Debug)]
pub enum RepoError {
    /// No row matched the lookup.
    NotFound,
    /// Input violates a foreign key, check, or not-null constraint.
    InvalidInput,
    /// A unique constraint was violated. Safe to log; not necessarily safe
    /// to show a caller verbatim, since it may name internal constraints.
    Conflict(String),
    /// Any other storage-backend failure (connection loss, a malformed
    /// query, etc.) — not something a caller can act on beyond retrying or
    /// failing the request.
    Backend(String),
}

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoError::InvalidInput => write!(f, "invalid repository input"),
            RepoError::NotFound => write!(f, "no matching row"),
            RepoError::Conflict(detail) => write!(f, "conflict: {detail}"),
            RepoError::Backend(detail) => write!(f, "storage backend error: {detail}"),
        }
    }
}

impl std::error::Error for RepoError {}
