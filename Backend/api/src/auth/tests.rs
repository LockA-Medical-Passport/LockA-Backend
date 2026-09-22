use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use domain::RepoError;
use ed25519_dalek::{Signer, SigningKey};
use soroban::{FakeSorobanRpcClient, xdr::*};
use std::{collections::HashMap, sync::Mutex};
use tower::ServiceExt;

const NETWORK: &str = "Test SDF Network ; September 2015";
#[derive(Default)]
struct MemoryChallenges(Mutex<HashMap<[u8; 32], (String, time::OffsetDateTime, bool)>>);
#[async_trait::async_trait]
impl ChallengeRepository for MemoryChallenges {
    async fn insert(
        &self,
        hash: &[u8; 32],
        account: &str,
        expiry: time::OffsetDateTime,
    ) -> Result<(), RepoError> {
        self.0.lock().unwrap().insert(*hash, (account.to_owned(), expiry, false));
        Ok(())
    }
    async fn consume(&self, hash: &[u8; 32], account: &str) -> Result<bool, RepoError> {
        let mut entries = self.0.lock().unwrap();
        let Some((stored, expiry, consumed)) = entries.get_mut(hash) else {
            return Ok(false);
        };
        if stored != account || *expiry <= time::OffsetDateTime::now_utc() || *consumed {
            return Ok(false);
        }
        *consumed = true;
        Ok(true)
    }
    async fn delete_expired(&self) -> Result<u64, RepoError> {
        Ok(0)
    }
}
fn setup() -> (Router, Arc<AuthState>, Arc<FakeSorobanRpcClient>, SigningKey, String) {
    let seed = format!("{}", stellar_strkey::ed25519::PrivateKey([7; 32]));
    let settings: config::Settings = serde_json::from_value(serde_json::json!({
        "database_url": "unused", "soroban_rpc_url": "http://localhost:8000/rpc", "stellar_network_passphrase": NETWORK,
        "object_storage_endpoint": "unused", "object_storage_bucket": "unused", "object_storage_access_key_id": "unused",
        "object_storage_secret_access_key": "unused", "jwt_signing_key": "test-jwt-secret-with-at-least-32-bytes",
        "sep10_signing_seed": seed, "sep10_home_domain": "example.com", "sep10_web_auth_domain": "auth.example.com",
        "sep10_web_auth_endpoint": "https://auth.example.com/auth/challenge"
    })).unwrap();
    let rpc = Arc::new(FakeSorobanRpcClient::default());
    let state = Arc::new(
        AuthState::new(&settings, rpc.clone(), Arc::new(MemoryChallenges::default())).unwrap(),
    );
    let key = SigningKey::from_bytes(&[9; 32]);
    let address = format!("{}", stellar_strkey::ed25519::PublicKey(key.verifying_key().to_bytes()));
    (crate::router(state.clone()), state, rpc, key, address)
}
async fn body(response: Response) -> serde_json::Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 32_768).await.unwrap()).unwrap()
}
async fn issue(app: &Router, address: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/auth/challenge?account={address}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(response.headers().contains_key("x-request-id"));
    body(response).await["transaction"].as_str().unwrap().to_owned()
}
fn sign(encoded: &str, key: &SigningKey) -> String {
    let TransactionEnvelope::Tx(mut envelope) =
        TransactionEnvelope::from_xdr_base64(encoded, Limits::none()).unwrap()
    else {
        panic!()
    };
    let hash = soroban::sep10::transaction_hash(&envelope.tx, NETWORK).unwrap();
    let public = key.verifying_key().to_bytes();
    let mut signatures = envelope.signatures.to_vec();
    signatures.push(DecoratedSignature {
        hint: SignatureHint(public[28..].try_into().unwrap()),
        signature: Signature(key.sign(&hash).to_bytes().to_vec().try_into().unwrap()),
    });
    envelope.signatures = signatures.try_into().unwrap();
    TransactionEnvelope::Tx(envelope).to_xdr_base64(Limits::none()).unwrap()
}
fn post(path: &str, transaction: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::json!({"transaction":transaction}).to_string()))
        .unwrap()
}

#[tokio::test]
async fn signed_challenge_issues_token_authenticates_and_rejects_replay() {
    let (app, _, rpc, key, address) = setup();
    let signed = sign(&issue(&app, &address).await, &key);
    rpc.push_response("loadAccount", &Option::<AccountEntry>::None);
    let response = app.clone().oneshot(post("/auth/verify", &signed)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let token = body(response).await["token"].as_str().unwrap().to_owned();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body(response).await["account"], address);
    rpc.push_response("loadAccount", &Option::<AccountEntry>::None);
    let replay = app.oneshot(post("/auth/verify", &signed)).await.unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body(replay).await["code"], "challenge_replayed");
}

#[tokio::test]
async fn same_endpoint_supports_sep10_form_submission_and_cors() {
    let (app, _, rpc, key, address) = setup();
    let signed = sign(&issue(&app, &address).await, &key);
    rpc.push_response("loadAccount", &Option::<AccountEntry>::None);
    let escaped = signed.replace('+', "%2B").replace('/', "%2F").replace('=', "%3D");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/challenge")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("transaction={escaped}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/auth/challenge")
                .header(header::ORIGIN, "https://wallet.example.com")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
}

#[tokio::test]
async fn concurrent_verify_issues_only_one_token() {
    let (app, _, rpc, key, address) = setup();
    let signed = sign(&issue(&app, &address).await, &key);
    for _ in 0..2 {
        rpc.push_response("loadAccount", &Option::<AccountEntry>::None);
    }
    let (a, b) = tokio::join!(
        app.clone().oneshot(post("/auth/verify", &signed)),
        app.oneshot(post("/auth/verify", &signed))
    );
    let mut statuses = [a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 401]);
}

#[tokio::test]
async fn invalid_signature_and_rpc_failure_do_not_consume_challenge() {
    let (app, _, rpc, key, address) = setup();
    let challenge = issue(&app, &address).await;
    rpc.push_response("loadAccount", &Option::<AccountEntry>::None);
    let bad = app
        .clone()
        .oneshot(post("/auth/verify", &sign(&challenge, &SigningKey::from_bytes(&[4; 32]))))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);
    rpc.push_error("loadAccount", soroban::RpcError::Unavailable);
    let signed = sign(&challenge, &key);
    assert_eq!(
        app.clone().oneshot(post("/auth/verify", &signed)).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    rpc.push_response("loadAccount", &Option::<AccountEntry>::None);
    assert_eq!(app.oneshot(post("/auth/verify", &signed)).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn rejects_expired_malformed_and_unissued_challenges() {
    let (app, state, rpc, key, address) = setup();
    let expired = state.sep10.build(&address, now() - 1000).unwrap();
    assert_eq!(
        app.clone()
            .oneshot(post("/auth/verify", &sign(&expired.transaction, &key)))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone().oneshot(post("/auth/verify", "garbage")).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert!(rpc.calls().is_empty());
    let unissued = state.sep10.build(&address, now()).unwrap();
    rpc.push_response("loadAccount", &Option::<AccountEntry>::None);
    assert_eq!(
        app.oneshot(post("/auth/verify", &sign(&unissued.transaction, &key)))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn rejects_missing_expired_forged_wrong_algorithm_issuer_and_audience_tokens() {
    let (app, state, _, _, address) = setup();
    assert_eq!(
        app.clone()
            .oneshot(Request::builder().uri("/auth/me").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let original = Claims {
        sub: address,
        iss: state.issuer.clone(),
        aud: state.home_domain.clone(),
        iat: now() - 10,
        exp: now() + 100,
    };
    let mut tokens = vec![
        "garbage".to_owned(),
        encode(&Header::new(Algorithm::HS256), &original, &EncodingKey::from_secret(b"wrong key"))
            .unwrap(),
        encode(&Header::new(Algorithm::HS384), &original, &state.encoding_key).unwrap(),
    ];
    let mut expired = original.clone();
    expired.exp = now() - 1;
    let mut issuer = original.clone();
    issuer.iss = "https://evil.example.com".into();
    let mut audience = original.clone();
    audience.aud = "evil.example.com".into();
    let mut future = original;
    future.iat = now() + 10;
    for claims in [expired, issuer, audience, future] {
        tokens.push(encode(&Header::new(Algorithm::HS256), &claims, &state.encoding_key).unwrap());
    }
    for token in tokens {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn query_validation_and_discovery() {
    let (app, state, _, _, address) = setup();
    for query in [
        "account=bad".to_owned(),
        format!("account={address}&home_domain=evil.com"),
        format!("account={address}&memo=1"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/auth/challenge?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(response).await["code"], "invalid_request");
    }
    let response = app
        .oneshot(Request::builder().uri("/.well-known/stellar.toml").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let text =
        String::from_utf8(to_bytes(response.into_body(), 4096).await.unwrap().to_vec()).unwrap();
    assert!(text.contains(&state.sep10.server_account()));
    assert!(text.contains(&state.issuer));
    assert!(!text.contains("test-jwt-secret"));
}
