pub mod auth;

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, Method, header},
    middleware,
    routing::get,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    ServiceBuilderExt,
    cors::{Any, CorsLayer},
    request_id::MakeRequestUuid,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

pub fn router(state: Arc<auth::AuthState>) -> Router {
    let protected = Router::new()
        .route("/auth/me", get(auth::me))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::authenticate));
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/auth/challenge", get(auth::challenge).post(auth::verify))
        .route("/auth/verify", axum::routing::post(auth::verify))
        .route("/.well-known/stellar.toml", get(auth::stellar_toml))
        .merge(protected)
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(20 * 1024))
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        .layer(CorsLayer::new().allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]))
        .layer(ServiceBuilder::new().set_x_request_id(MakeRequestUuid)
            .layer(TraceLayer::new_for_http().make_span_with(|request: &Request| {
                let request_id = request.headers().get("x-request-id")
                    .and_then(|value| value.to_str().ok()).unwrap_or("unknown");
                tracing::info_span!("http_request", method = %request.method(), path = %request.uri().path(), request_id)
            })).propagate_x_request_id())
}
