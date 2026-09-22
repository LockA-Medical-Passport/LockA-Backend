//! Single, typed configuration layer for the LockA Backend service.
//!
//! Call [`Settings::load`] once at process startup, before doing anything
//! else that depends on configuration. It loads a `.env` file if one is
//! present (for local development; ignored if missing), then deserializes
//! the process environment into [`Settings`], failing with a clear,
//! specific error if a required variable is missing or a value can't be
//! parsed into its expected type.
//!
//! `api` and `worker` currently load the same [`Settings`]: they're one
//! logical service sharing one database, chain endpoint, and set of
//! secrets, so a single required-config surface keeps both binaries
//! failing fast on the same missing variable rather than drifting apart.

use serde::Deserialize;

fn default_api_bind_addr() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_worker_tick_interval_secs() -> u64 {
    30
}

fn default_database_max_connections() -> u32 {
    10
}

fn default_database_timeout_secs() -> u64 {
    10
}
fn default_rpc_timeout_secs() -> u64 {
    15
}
fn default_rpc_max_retries() -> u32 {
    3
}
fn default_auth_challenge_ttl_secs() -> u64 {
    900
}
fn default_auth_token_ttl_secs() -> u64 {
    900
}

/// Typed application configuration, sourced from environment variables.
///
/// See `Backend/.env.example` for the full list of variables, defaults,
/// and example values.
#[derive(Deserialize, Clone)]
pub struct Settings {
    /// PostgreSQL connection string.
    pub database_url: String,
    /// Maximum number of connections the pool opens to PostgreSQL.
    #[serde(default = "default_database_max_connections")]
    pub database_max_connections: u32,
    /// Deadline for acquiring a connection or completing startup migrations.
    #[serde(default = "default_database_timeout_secs")]
    pub database_timeout_secs: u64,
    #[serde(default = "default_rpc_timeout_secs")]
    pub soroban_rpc_timeout_secs: u64,
    #[serde(default = "default_rpc_max_retries")]
    pub soroban_rpc_max_retries: u32,
    /// Soroban RPC endpoint (e.g. `https://soroban-testnet.stellar.org`).
    pub soroban_rpc_url: String,
    /// Stellar network passphrase the service operates against.
    pub stellar_network_passphrase: String,
    /// S3-compatible endpoint for encrypted off-chain record storage.
    pub object_storage_endpoint: String,
    /// Bucket used for encrypted off-chain record storage.
    pub object_storage_bucket: String,
    /// Access key ID for the object storage endpoint. Secret.
    pub object_storage_access_key_id: String,
    /// Secret access key for the object storage endpoint. Secret.
    pub object_storage_secret_access_key: String,
    /// Signing key for issued session JWTs (see the SEP-10 auth flow). Secret.
    pub jwt_signing_key: String,
    /// Server-only Stellar seed; never a patient/provider key.
    pub sep10_signing_seed: String,
    pub sep10_home_domain: String,
    pub sep10_web_auth_domain: String,
    /// Public canonical auth endpoint URI (also the JWT issuer).
    pub sep10_web_auth_endpoint: String,
    #[serde(default = "default_auth_challenge_ttl_secs")]
    pub auth_challenge_ttl_secs: u64,
    #[serde(default = "default_auth_token_ttl_secs")]
    pub auth_token_ttl_secs: u64,

    /// Address the `api` HTTP server binds to.
    #[serde(default = "default_api_bind_addr")]
    pub api_bind_addr: String,
    /// How often the `worker` polls for new work, in seconds.
    #[serde(default = "default_worker_tick_interval_secs")]
    pub worker_tick_interval_secs: u64,
}

/// Fields whose values must never be printed in full (logs, error messages, etc).
const REDACTED: &str = "[REDACTED]";

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("database_url", &REDACTED)
            .field("database_max_connections", &self.database_max_connections)
            .field("soroban_rpc_url", &self.soroban_rpc_url)
            .field("stellar_network_passphrase", &self.stellar_network_passphrase)
            .field("object_storage_endpoint", &self.object_storage_endpoint)
            .field("object_storage_bucket", &self.object_storage_bucket)
            .field("object_storage_access_key_id", &REDACTED)
            .field("object_storage_secret_access_key", &REDACTED)
            .field("jwt_signing_key", &REDACTED)
            .field("sep10_signing_seed", &REDACTED)
            .field("api_bind_addr", &self.api_bind_addr)
            .field("worker_tick_interval_secs", &self.worker_tick_interval_secs)
            .finish()
    }
}

/// A configuration-loading failure: a missing/invalid `.env` file, a
/// missing required environment variable, or a value that couldn't be
/// parsed into its expected type.
#[derive(Debug)]
pub enum ConfigError {
    Invalid(&'static str),
    Dotenv(dotenvy::Error),
    Env(envy::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Invalid(message) => write!(f, "invalid configuration: {message}"),
            ConfigError::Dotenv(err) => write!(f, "failed to load .env file: {err}"),
            ConfigError::Env(envy::Error::MissingValue(field)) => {
                write!(f, "missing required environment variable: {}", field.to_uppercase())
            }
            ConfigError::Env(envy::Error::Custom(msg)) => write!(f, "invalid configuration: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Settings {
    /// Loads configuration from a `.env` file (if present) and the process
    /// environment.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if a `.env` file exists but can't be parsed,
    /// a required variable is missing, or a variable's value can't be
    /// parsed into its expected type. Callers should treat any error here
    /// as fatal: log it and exit, don't attempt to run with partial
    /// configuration.
    pub fn load() -> Result<Self, ConfigError> {
        load_dotenv()?;
        from_iter(std::env::vars())
    }
}

fn load_dotenv() -> Result<(), ConfigError> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        // No `.env` file is the normal case outside local development
        // (e.g. in CI or production, where real env vars are set directly).
        Err(err) if err.not_found() => Ok(()),
        Err(err) => Err(ConfigError::Dotenv(err)),
    }
}

fn from_iter(iter: impl IntoIterator<Item = (String, String)>) -> Result<Settings, ConfigError> {
    let settings: Settings = envy::from_iter(iter).map_err(ConfigError::Env)?;
    if settings.database_max_connections == 0 || settings.database_timeout_secs == 0 {
        return Err(ConfigError::Invalid("database pool size and timeout must be positive"));
    }
    if settings.worker_tick_interval_secs == 0
        || settings.soroban_rpc_timeout_secs == 0
        || settings.soroban_rpc_max_retries > 5
    {
        return Err(ConfigError::Invalid("intervals must be positive and RPC retries at most 5"));
    }
    if settings.jwt_signing_key.len() < 32 {
        return Err(ConfigError::Invalid("JWT_SIGNING_KEY must contain at least 32 bytes"));
    }
    if !(1..=900).contains(&settings.auth_challenge_ttl_secs)
        || !(1..=3600).contains(&settings.auth_token_ttl_secs)
    {
        return Err(ConfigError::Invalid(
            "challenge TTL must be 1..=900 and token TTL 1..=3600 seconds",
        ));
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_vars() -> Vec<(String, String)> {
        vec![
            ("DATABASE_URL".into(), "postgres://user:pass@localhost/locka".into()),
            ("SOROBAN_RPC_URL".into(), "https://soroban-testnet.stellar.org".into()),
            ("STELLAR_NETWORK_PASSPHRASE".into(), "Test SDF Network ; September 2015".into()),
            ("OBJECT_STORAGE_ENDPOINT".into(), "https://storage.example.com".into()),
            ("OBJECT_STORAGE_BUCKET".into(), "locka-records".into()),
            ("OBJECT_STORAGE_ACCESS_KEY_ID".into(), "test-access-key".into()),
            ("OBJECT_STORAGE_SECRET_ACCESS_KEY".into(), "test-secret-key".into()),
            ("SEP10_SIGNING_SEED".into(), "secret-server-seed".into()),
            ("SEP10_HOME_DOMAIN".into(), "localhost".into()),
            ("SEP10_WEB_AUTH_DOMAIN".into(), "localhost".into()),
            ("SEP10_WEB_AUTH_ENDPOINT".into(), "http://localhost:8080/auth/challenge".into()),
            ("JWT_SIGNING_KEY".into(), "test-jwt-signing-key-at-least-32-bytes".into()),
        ]
    }

    #[test]
    fn loads_with_all_required_vars_and_default_optionals() {
        let settings = from_iter(base_vars()).expect("should load with all required vars set");

        assert_eq!(settings.database_url, "postgres://user:pass@localhost/locka");
        assert_eq!(settings.database_max_connections, 10);
        assert_eq!(settings.api_bind_addr, "0.0.0.0:8080");
        assert_eq!(settings.worker_tick_interval_secs, 30);
    }

    #[test]
    fn optional_vars_override_their_defaults() {
        let mut vars = base_vars();
        vars.push(("API_BIND_ADDR".into(), "127.0.0.1:9000".into()));
        vars.push(("WORKER_TICK_INTERVAL_SECS".into(), "5".into()));
        vars.push(("DATABASE_MAX_CONNECTIONS".into(), "25".into()));

        let settings = from_iter(vars).expect("should load");

        assert_eq!(settings.api_bind_addr, "127.0.0.1:9000");
        assert_eq!(settings.worker_tick_interval_secs, 5);
        assert_eq!(settings.database_max_connections, 25);
    }

    #[test]
    fn fails_fast_with_a_clear_message_when_a_required_var_is_missing() {
        let mut vars = base_vars();
        vars.retain(|(key, _)| key != "DATABASE_URL");

        let err = from_iter(vars).expect_err("should fail without DATABASE_URL");

        assert_eq!(err.to_string(), "missing required environment variable: DATABASE_URL");
    }

    #[test]
    fn fails_with_a_clear_message_when_a_value_is_malformed() {
        let mut vars = base_vars();
        vars.push(("WORKER_TICK_INTERVAL_SECS".into(), "not-a-number".into()));

        let err = from_iter(vars).expect_err("should fail to parse a non-numeric interval");

        assert!(err.to_string().starts_with("invalid configuration:"), "unexpected message: {err}");
    }

    #[test]
    fn debug_output_redacts_secrets() {
        let settings = from_iter(base_vars()).expect("should load");
        let debug_output = format!("{settings:?}");

        assert!(!debug_output.contains("test-secret-key"));
        assert!(!debug_output.contains("secret-server-seed"));
        assert!(!debug_output.contains("test-access-key"));
        assert!(!debug_output.contains("test-jwt-signing-key"));
        assert!(!debug_output.contains("pass@localhost"));
        // Non-secret fields still show through, so the output stays useful.
        assert!(debug_output.contains("soroban-testnet.stellar.org"));
    }
    #[test]
    fn rejects_invalid_pool_retry_and_auth_limits() {
        for (name, value) in [
            ("DATABASE_MAX_CONNECTIONS", "0"),
            ("DATABASE_TIMEOUT_SECS", "0"),
            ("SOROBAN_RPC_TIMEOUT_SECS", "0"),
            ("SOROBAN_RPC_MAX_RETRIES", "6"),
            ("WORKER_TICK_INTERVAL_SECS", "0"),
            ("JWT_SIGNING_KEY", "short"),
            ("AUTH_CHALLENGE_TTL_SECS", "901"),
            ("AUTH_TOKEN_TTL_SECS", "3601"),
        ] {
            let mut vars = base_vars();
            vars.retain(|(key, _)| key != name);
            vars.push((name.into(), value.into()));
            assert!(from_iter(vars).is_err(), "accepted invalid {name}");
        }
    }
}
