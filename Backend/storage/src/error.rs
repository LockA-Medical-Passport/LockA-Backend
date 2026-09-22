use domain::RepoError;

/// The Postgres SQLSTATE for a unique-constraint violation.
const UNIQUE_VIOLATION: &str = "23505";

/// Maps a raw `sqlx::Error` into the [`RepoError`] every repository method
/// returns — the one place in this crate that's allowed to name
/// `sqlx::Error`, so it can't leak into `domain`'s public API.
pub(crate) fn map_sqlx_error(err: sqlx::Error) -> RepoError {
    match &err {
        sqlx::Error::RowNotFound => RepoError::NotFound,
        sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some(UNIQUE_VIOLATION) => {
            RepoError::Conflict(db_err.message().to_string())
        }
        sqlx::Error::Database(db_err)
            if matches!(db_err.code().as_deref(), Some("23503" | "23514" | "23502")) =>
        {
            RepoError::InvalidInput
        }
        _ => RepoError::Backend(err.to_string()),
    }
}
