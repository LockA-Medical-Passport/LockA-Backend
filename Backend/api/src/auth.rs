use axum::{
    Extension, Form, Json,
    extract::{FromRequest, Query, Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::ChallengeRepository;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use soroban::{
    SorobanRpc,
    sep10::{ChallengeError, Sep10},
};
use std::sync::Arc;

pub struct AuthState {
    sep10: Sep10,
    rpc: Arc<dyn SorobanRpc>,
    challenges: Arc<dyn ChallengeRepository>,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    network: String,
    home_domain: String,
    token_ttl: u64,
}

impl AuthState {
    pub fn new(
        settings: &config::Settings,
        rpc: Arc<dyn SorobanRpc>,
        challenges: Arc<dyn ChallengeRepository>,
    ) -> Result<Self, ChallengeError> {
        if settings.jwt_signing_key.len() < 32
            || !(1..=3600).contains(&settings.auth_token_ttl_secs)
        {
            return Err(ChallengeError::Configuration);
        }
        Ok(Self {
            sep10: Sep10::from_settings(settings)?,
            rpc,
            challenges,
            encoding_key: EncodingKey::from_secret(settings.jwt_signing_key.as_bytes()),
            decoding_key: DecodingKey::from_secret(settings.jwt_signing_key.as_bytes()),
            issuer: settings.sep10_web_auth_endpoint.clone(),
            network: settings.stellar_network_passphrase.clone(),
            home_domain: settings.sep10_home_domain.clone(),
            token_ttl: settings.auth_token_ttl_secs,
        })
    }
    fn issue_token(&self, account: &str, now: u64) -> Result<String, ApiError> {
        let claims = Claims {
            sub: account.to_owned(),
            iss: self.issuer.clone(),
            aud: self.home_domain.clone(),
            iat: now,
            exp: now + self.token_ttl,
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding_key)
            .map_err(|_| ApiError::internal())
    }
    fn validate_token(&self, token: &str) -> Result<Claims, ApiError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.home_domain]);
        validation.set_required_spec_claims(&["exp", "iat", "iss", "sub", "aud"]);
        validation.leeway = 0;
        let claims = decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|_| ApiError::unauthorized("invalid_token", "token is invalid or expired"))?
            .claims;
        if claims.iat > now()
            || claims.exp <= now()
            || claims.exp <= claims.iat
            || claims.exp - claims.iat > self.token_ttl
        {
            return Err(ApiError::unauthorized("invalid_token", "token is invalid or expired"));
        }
        Ok(claims)
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}
impl ApiError {
    fn bad_request(message: &'static str) -> Self {
        Self { status: StatusCode::BAD_REQUEST, code: "invalid_request", message }
    }
    fn unauthorized(code: &'static str, message: &'static str) -> Self {
        Self { status: StatusCode::UNAUTHORIZED, code, message }
    }
    fn unavailable() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "dependency_unavailable",
            message: "authentication dependency unavailable; try again later",
        }
    }
    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: "authentication could not be completed",
        }
    }
}
impl From<ChallengeError> for ApiError {
    fn from(error: ChallengeError) -> Self {
        match error {
            ChallengeError::Configuration => Self::internal(),
            ChallengeError::Account => Self::bad_request(
                "account must be a valid Stellar G-address distinct from the server",
            ),
            ChallengeError::Malformed => Self::bad_request("malformed challenge transaction"),
            ChallengeError::TimeBounds => {
                Self::unauthorized("challenge_expired", "challenge has expired or is not yet valid")
            }
            ChallengeError::Contents => Self::unauthorized(
                "invalid_challenge",
                "challenge source, domain, or operations are invalid",
            ),
            ChallengeError::Signature => Self::unauthorized(
                "invalid_signature",
                "challenge signatures do not authorize this account",
            ),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({"code": self.code, "message": self.message, "error": self.message}))).into_response()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeQuery {
    account: String,
    home_domain: Option<String>,
}
#[derive(Serialize)]
pub struct ChallengeResponse {
    transaction: String,
    network_passphrase: String,
}

pub async fn challenge(
    State(state): State<Arc<AuthState>>,
    query: Result<Query<ChallengeQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<ChallengeResponse>, ApiError> {
    let Query(query) = query.map_err(|_| {
        ApiError::bad_request(
            "provide account and optional home_domain; memo and client_domain are unsupported",
        )
    })?;
    if query.home_domain.as_ref().is_some_and(|domain| domain != &state.home_domain) {
        return Err(ApiError::bad_request("home_domain does not match this server"));
    }
    let challenge = state.sep10.build(&query.account, now())?;
    let expires_at = time::OffsetDateTime::from_unix_timestamp(challenge.expires_at as i64)
        .map_err(|_| ApiError::internal())?;
    state
        .challenges
        .insert(&challenge.hash, &query.account, expires_at)
        .await
        .map_err(|_| ApiError::unavailable())?;
    Ok(Json(ChallengeResponse {
        transaction: challenge.transaction,
        network_passphrase: state.network.clone(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyRequest {
    transaction: String,
}
impl<S: Send + Sync> FromRequest<S> for VerifyRequest {
    type Rejection = ApiError;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        match content_type {
            "application/json" => Json::<Self>::from_request(request, state)
                .await
                .map(|Json(value)| value)
                .map_err(|_| {
                    ApiError::bad_request("expected a JSON object with a transaction XDR string")
                }),
            "application/x-www-form-urlencoded" => {
                Form::<Self>::from_request(request, state).await.map(|Form(value)| value).map_err(
                    |_| ApiError::bad_request("expected a form with a transaction XDR string"),
                )
            }
            _ => Err(ApiError {
                status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
                code: "unsupported_media_type",
                message: "use application/json or application/x-www-form-urlencoded",
            }),
        }
    }
}

#[derive(Serialize)]
pub struct TokenResponse {
    token: String,
    token_type: &'static str,
    expires_in: u64,
}
pub async fn verify(
    State(state): State<Arc<AuthState>>,
    input: VerifyRequest,
) -> Result<Json<TokenResponse>, ApiError> {
    let challenge = state.sep10.parse(&input.transaction, now())?;
    let account =
        state.rpc.load_account(&challenge.account).await.map_err(|_| ApiError::unavailable())?;
    challenge.verify_signatures(account.as_ref())?;
    // Prepare the token before consuming: a local encoding failure must not
    // burn a valid challenge. Only the atomic database winner returns it.
    let token = state.issue_token(&challenge.account, now())?;
    if !state
        .challenges
        .consume(&challenge.hash, &challenge.account)
        .await
        .map_err(|_| ApiError::unavailable())?
    {
        return Err(ApiError::unauthorized(
            "challenge_replayed",
            "challenge is unknown, expired, or already used",
        ));
    }
    Ok(Json(TokenResponse { token, token_type: "Bearer", expires_in: state.token_ttl }))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct AuthenticatedAccount {
    pub account: String,
}

/// Apply this middleware to every protected router; handlers consume the
/// authenticated identity from Extension<AuthenticatedAccount>.
pub async fn authenticate(
    State(state): State<Arc<AuthState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("missing_token", "a Bearer token is required"))?;
    let (scheme, token) = value
        .split_once(' ')
        .filter(|(scheme, token)| scheme.eq_ignore_ascii_case("Bearer") && !token.is_empty())
        .ok_or_else(|| ApiError::unauthorized("invalid_token", "a Bearer token is required"))?;
    let _ = scheme;
    let claims = state.validate_token(token)?;
    request.extensions_mut().insert(AuthenticatedAccount { account: claims.sub });
    Ok(next.run(request).await)
}

pub async fn me(Extension(account): Extension<AuthenticatedAccount>) -> Json<AuthenticatedAccount> {
    Json(account)
}

pub async fn stellar_toml(State(state): State<Arc<AuthState>>) -> impl IntoResponse {
    // JSON string quoting is compatible with TOML basic strings for validated
    // ASCII URLs/passphrases and avoids interpolating unescaped configuration.
    let quote = |value: &str| serde_json::to_string(value).expect("serialize string");
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!(
            "VERSION=\"2.0.0\"\nSIGNING_KEY={}\nNETWORK_PASSPHRASE={}\nWEB_AUTH_ENDPOINT={}\n",
            quote(&state.sep10.server_account()),
            quote(&state.network),
            quote(&state.issuer)
        ),
    )
}

fn now() -> u64 {
    time::OffsetDateTime::now_utc().unix_timestamp().max(0) as u64
}

#[cfg(test)]
mod tests;
