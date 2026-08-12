use std::time::Duration;

use tokio::time::{MissedTickBehavior, interval};

#[tokio::main]
async fn main() {
    telemetry::init();

    let settings = config::Settings::load().unwrap_or_else(|err| {
        tracing::error!(%err, "invalid configuration");
        std::process::exit(1);
    });

    let tick_interval_secs = settings.worker_tick_interval_secs;
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
