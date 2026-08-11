//! Shared `tracing` subscriber setup for LockA Backend binaries.
//!
//! Call [`init`] once at process startup, before doing anything else.
//!
//! - Level filtering is controlled by the standard `RUST_LOG` environment
//!   variable (defaults to `info` when unset).
//! - Output format is controlled by `LOG_FORMAT`: `json` for production,
//!   anything else (including unset) for human-readable local development.
//! - Every span opened with `#[tracing::instrument]` anywhere in the
//!   workspace gets a `close` event carrying `time.busy` / `time.idle`
//!   fields, so instrumenting a function is enough to make its duration
//!   observable without any per-call-site timing code.

use std::env;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

const LOG_FORMAT_ENV_VAR: &str = "LOG_FORMAT";

/// Initializes the global `tracing` subscriber for the current process.
///
/// # Panics
///
/// Panics if a global subscriber has already been installed, or if
/// `RUST_LOG` is set to an invalid filter directive.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json_output =
        env::var(LOG_FORMAT_ENV_VAR).is_ok_and(|value| value.eq_ignore_ascii_case("json"));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_span_events(FmtSpan::CLOSE);

    if json_output {
        subscriber.json().init();
    } else {
        subscriber.pretty().init();
    }
}
