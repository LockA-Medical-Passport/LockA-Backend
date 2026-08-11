use std::time::Duration;

use tokio::time::{MissedTickBehavior, interval};

const DEFAULT_TICK_INTERVAL_SECS: u64 = 30;

#[tokio::main]
async fn main() {
    telemetry::init();

    let tick_interval_secs = std::env::var("WORKER_TICK_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_TICK_INTERVAL_SECS);

    tracing::info!(tick_interval_secs, "starting worker");

    let mut ticker = interval(Duration::from_secs(tick_interval_secs));
    // Don't try to "catch up" with a burst of missed ticks after a stall
    // (e.g. the process was paused); just resume on the regular cadence.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = ticker.tick() => tick().await,
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutdown signal received");
                break;
            }
        }
    }
}

/// One iteration of the worker's background loop.
///
/// Currently a placeholder: real Soroban contract-event indexing lands in
/// issue #20. `#[tracing::instrument]` is enough to make each run's
/// duration observable in logs (see `telemetry::init`), which is the
/// pattern future indexing work should follow.
#[tracing::instrument]
async fn tick() {
    tracing::info!("worker tick");
}
