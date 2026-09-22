//! PostgreSQL access (via `sqlx`) and encrypted object storage integrations
//! (see issue #9 onward).
//!
//! Instrumentation convention: annotate every query-executing function with
//! `#[tracing::instrument(skip(pool, ..))]` (skip the connection pool and
//! any large/sensitive arguments). The subscriber configured in
//! `telemetry::init` records a `close` event with `time.busy` / `time.idle`
//! for every span, so instrumenting a function is all that's needed to
//! make its latency observable — no manual timing code.

mod auth;
pub use auth::PgChallengeRepository;

mod consent;
mod device;
mod error;
mod patient;
mod provider;
mod record;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub use consent::PgConsentRepository;
pub use device::PgDeviceRepository;
pub use patient::PgPatientRepository;
pub use provider::{PgProviderRepository, PgProviderStaffRepository};
pub use record::PgRecordIndexRepository;

/// Embedded migrations from `storage/migrations/`, applied by [`connect`].
///
/// `sqlx::migrate!()` resolves that path relative to this crate's
/// `CARGO_MANIFEST_DIR` at compile time, so the SQL files are baked into the
/// binary — a deployed `api`/`worker` never needs the `.sql` files on disk,
/// and `sqlx-cli` (a separate, manually-installed tool) is only needed for
/// authoring new migrations locally, not for applying them at runtime.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

/// Connects a pooled client to `settings.database_url`, applies any pending
/// migrations, and confirms the connection with a trivial query.
///
/// Safe to call concurrently from both `api` and `worker` at startup:
/// `sqlx`'s migrator takes a Postgres advisory lock for the duration of
/// applying migrations, so two processes racing to call this on a fresh
/// database serialize automatically rather than corrupting each other's
/// migration state — no leader-election needed between the two binaries.
///
/// # Errors
///
/// Returns [`DbError`] if the pool can't be established, a migration fails
/// to apply, or the post-migration health check fails. Callers should treat
/// any error here as fatal: log it and exit, the same way a
/// [`config::ConfigError`] is already handled.
#[tracing::instrument(skip_all)]
pub async fn connect(settings: &config::Settings) -> Result<PgPool, DbError> {
    if settings.database_max_connections == 0 || settings.database_timeout_secs == 0 {
        return Err(DbError::InvalidPoolConfiguration);
    }
    let timeout = std::time::Duration::from_secs(settings.database_timeout_secs);
    let pool = PgPoolOptions::new()
        .acquire_timeout(timeout)
        .max_connections(settings.database_max_connections)
        .connect(&settings.database_url)
        .await
        .map_err(DbError::Connect)?;

    tokio::time::timeout(timeout, MIGRATOR.run(&pool))
        .await
        .map_err(|_| DbError::StartupTimeout)?
        .map_err(DbError::Migrate)?;

    // Deliberately schema-independent (touches no table), so it verifies the
    // pool can actually run a query post-migration without assuming any
    // particular table exists yet.
    tokio::time::timeout(timeout, sqlx::query!(r#"SELECT 1 AS "one!: i32""#).fetch_one(&pool))
        .await
        .map_err(|_| DbError::StartupTimeout)?
        .map_err(DbError::HealthCheck)?;

    Ok(pool)
}

/// A database startup failure: could not connect, could not apply pending
/// migrations, or the post-migration health check failed.
#[derive(Debug)]
pub enum DbError {
    InvalidPoolConfiguration,
    StartupTimeout,
    Connect(sqlx::Error),
    Migrate(sqlx::migrate::MigrateError),
    HealthCheck(sqlx::Error),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::InvalidPoolConfiguration => {
                write!(f, "database pool size and timeout must be positive")
            }
            DbError::StartupTimeout => write!(f, "database startup deadline exceeded"),
            DbError::Connect(err) => write!(f, "failed to connect to the database: {err}"),
            DbError::Migrate(err) => write!(f, "failed to apply database migrations: {err}"),
            DbError::HealthCheck(err) => write!(f, "database health check failed: {err}"),
        }
    }
}

impl std::error::Error for DbError {}

#[cfg(test)]
mod tests {
    /// A fresh, migrated database per test — sqlx creates and tears it down
    /// automatically from the `DATABASE_URL` this test binary was compiled
    /// against, applying the migrations in `storage/migrations/` (the same
    /// ones `MIGRATOR` embeds) before the test body runs.
    #[sqlx::test]
    async fn migrations_apply_cleanly_and_create_the_expected_tables(pool: sqlx::PgPool) {
        let expected_tables = [
            "auth_challenges",
            "patients",
            "providers",
            "provider_staff",
            "access_requests",
            "consent_grants",
            "record_index",
            "device_registrations",
            "device_readings_index",
            "notification_channels",
            "notifications",
            "audit_log_index",
            "indexer_checkpoints",
        ];

        for table in expected_tables {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
                 WHERE table_schema = 'public' AND table_name = $1)",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|err| panic!("failed to check for table {table}: {err}"));

            assert!(exists, "expected migration to create table `{table}`");
        }

        let expected_enums = ["patient_status", "record_category", "audit_event_type"];

        for enum_name in expected_enums {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_type WHERE typname = $1)")
                    .bind(enum_name)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or_else(|err| panic!("failed to check for enum {enum_name}: {err}"));

            assert!(exists, "expected migration to create enum type `{enum_name}`");
        }
    }
    fn settings(database_url: String) -> config::Settings {
        serde_json::from_value(serde_json::json!({
            "database_url": database_url, "database_timeout_secs": 1, "database_max_connections": 2,
            "soroban_rpc_url": "unused", "stellar_network_passphrase": "unused",
            "object_storage_endpoint": "unused", "object_storage_bucket": "unused",
            "object_storage_access_key_id": "unused", "object_storage_secret_access_key": "unused",
            "jwt_signing_key": "unused", "sep10_signing_seed": "unused",
            "sep10_home_domain": "unused", "sep10_web_auth_domain": "unused", "sep10_web_auth_endpoint": "unused"
        })).unwrap()
    }

    #[tokio::test]
    async fn unreachable_database_fails_within_configured_deadline() {
        let config = settings("postgres://invalid:invalid@127.0.0.1:1/unreachable".into());
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(3), super::connect(&config)).await;
        assert!(matches!(result, Ok(Err(super::DbError::Connect(_)))));
    }

    #[sqlx::test]
    async fn startup_pool_uses_configuration_and_rejects_changed_migration(pool: sqlx::PgPool) {
        use sqlx::ConnectOptions;
        let config = settings(pool.connect_options().to_url_lossy().to_string());
        let connected = super::connect(&config).await.unwrap();
        assert_eq!(connected.options().get_max_connections(), 2);
        connected.close().await;
        // Simulate a deployed database with a changed applied migration.
        sqlx::query("UPDATE _sqlx_migrations SET checksum = $1")
            .bind(vec![0u8; 48])
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(super::connect(&config).await, Err(super::DbError::Migrate(_))));
    }
}
