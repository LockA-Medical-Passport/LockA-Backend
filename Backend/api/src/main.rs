use domain::ChallengeRepository;
use soroban::SorobanRpc;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    telemetry::init();

    let settings = config::Settings::load().unwrap_or_else(|err| {
        tracing::error!(%err, "invalid configuration");
        std::process::exit(1);
    });

    // Migrate and health-check before accepting traffic.
    let pool = storage::connect(&settings).await.unwrap_or_else(|err| {
        tracing::error!(%err, "database unavailable");
        std::process::exit(1);
    });

    let rpc = Arc::new(soroban::SorobanRpcClient::from_settings(&settings).unwrap_or_else(|err| {
        tracing::error!(%err, "invalid RPC configuration");
        std::process::exit(1);
    }));
    rpc.get_network().await.unwrap_or_else(|err| {
        tracing::error!(%err, "Stellar network unavailable or mismatched");
        std::process::exit(1);
    });
    let challenges = Arc::new(storage::PgChallengeRepository::new(pool));
    let state =
        api::auth::AuthState::new(&settings, rpc, challenges.clone()).unwrap_or_else(|err| {
            tracing::error!(%err, "invalid authentication configuration");
            std::process::exit(1);
        });
    let app = api::router(Arc::new(state));

    let bind_addr = &settings.api_bind_addr;
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .unwrap_or_else(|err| panic!("failed to bind {bind_addr}: {err}"));

    tracing::info!(addr = %bind_addr, "starting API server");

    let cleanup = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(err) = challenges.delete_expired().await {
                tracing::error!(%err, "expired challenge cleanup failed");
            }
        }
    });
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("API server failed");
    cleanup.abort();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    tracing::info!("shutdown signal received");
}
