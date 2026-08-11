use axum::Router;
use axum::extract::Request;
use axum::routing::get;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;
use tower_http::request_id::MakeRequestUuid;
use tower_http::trace::TraceLayer;
use tracing::info_span;

const REQUEST_ID_HEADER: &str = "x-request-id";
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

#[tokio::main]
async fn main() {
    telemetry::init();

    let app = Router::new().route("/healthz", get(health)).layer(
        ServiceBuilder::new()
            // Assign a request id before the request reaches `TraceLayer`, so
            // it can be attached to the request's tracing span.
            .set_x_request_id(MakeRequestUuid)
            .layer(TraceLayer::new_for_http().make_span_with(request_span))
            // Echo the request id back on the response before it leaves `TraceLayer`.
            .propagate_x_request_id(),
    );

    let bind_addr =
        std::env::var("API_BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|err| panic!("failed to bind {bind_addr}: {err}"));

    tracing::info!(addr = %bind_addr, "starting API server");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("API server failed");
}

fn request_span(request: &Request) -> tracing::Span {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown");

    info_span!(
        "http_request",
        method = %request.method(),
        path = %request.uri().path(),
        request_id = %request_id,
    )
}

async fn health() -> &'static str {
    "ok"
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    tracing::info!("shutdown signal received");
}
