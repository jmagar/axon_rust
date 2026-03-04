use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;

pub(crate) const OAUTH_SESSION_COOKIE: &str = "__Host-axon_oauth_session";
pub(crate) const OAUTH_SESSION_TTL_SECS: u64 = 60 * 60 * 24 * 7;
pub(crate) const OAUTH_REFRESH_TTL_SECS: u64 = 60 * 60 * 24 * 30;

#[derive(Clone)]
pub(crate) struct GoogleOAuthState {
    pub(crate) inner: std::sync::Arc<GoogleOAuthInner>,
}

pub(crate) struct GoogleOAuthInner {
    pub(crate) config: Option<GoogleOAuthConfig>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) redis_client: Option<redis::Client>,
    pub(crate) pending_state: Mutex<HashMap<String, PendingStateRecord>>,
    pub(crate) oauth_sessions: Mutex<HashMap<String, GoogleTokenResponse>>,
    pub(crate) oauth_clients: Mutex<HashMap<String, RegisteredClient>>,
    pub(crate) auth_codes: Mutex<HashMap<String, AuthCodeRecord>>,
    pub(crate) access_tokens: Mutex<HashMap<String, AccessTokenRecord>>,
    pub(crate) refresh_tokens: Mutex<HashMap<String, RefreshTokenRecord>>,
    pub(crate) rate_limits: Mutex<HashMap<String, RateLimitRecord>>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct GoogleOAuthConfig {
    pub(crate) auth_url: String,
    pub(crate) token_url: String,
    pub(crate) broker_issuer: String,
    pub(crate) authorization_endpoint: String,
    pub(crate) token_endpoint: String,
    pub(crate) registration_endpoint: String,
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) redirect_uri: String,
    pub(crate) resource_server_url: String,
    pub(crate) resource_metadata_url: String,
    pub(crate) redis_key_prefix: String,
    pub(crate) scopes: Vec<String>,
    pub(crate) dcr_token: Option<String>,
    pub(crate) redirect_policy: RedirectPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GoogleTokenResponse {
    pub(crate) access_token: String,
    pub(crate) token_type: String,
    pub(crate) expires_in: Option<i64>,
    pub(crate) refresh_token: Option<String>,
    pub(crate) scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CallbackParams {
    pub(crate) code: Option<String>,
    pub(crate) state: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct LoginQuery {
    pub(crate) return_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OAuthStatus {
    pub(crate) configured: bool,
    pub(crate) authenticated: bool,
    pub(crate) redirect_uri: Option<String>,
    pub(crate) scopes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OAuthError<'a> {
    pub(crate) error: &'a str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RegisteredClient {
    pub(crate) redirect_uris: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AuthCodeRecord {
    pub(crate) client_id: String,
    pub(crate) redirect_uri: String,
    pub(crate) scope: String,
    pub(crate) code_challenge: Option<String>,
    pub(crate) code_challenge_method: Option<String>,
    pub(crate) expires_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AccessTokenRecord {
    pub(crate) scope: String,
    pub(crate) expires_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RefreshTokenRecord {
    pub(crate) client_id: String,
    pub(crate) scope: String,
    pub(crate) expires_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PendingStateRecord {
    pub(crate) return_to: String,
    pub(crate) expires_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RateLimitRecord {
    pub(crate) count: u64,
    pub(crate) reset_at_unix: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) enum RedirectPolicy {
    Any,
    LoopbackOnly,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProtectedResourceMetadata {
    pub(crate) resource: String,
    pub(crate) authorization_servers: Vec<String>,
    pub(crate) scopes_supported: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthorizationServerMetadata {
    pub(crate) issuer: String,
    pub(crate) authorization_endpoint: String,
    pub(crate) token_endpoint: String,
    pub(crate) registration_endpoint: String,
    pub(crate) response_types_supported: Vec<String>,
    pub(crate) grant_types_supported: Vec<String>,
    pub(crate) token_endpoint_auth_methods_supported: Vec<String>,
    pub(crate) code_challenge_methods_supported: Vec<String>,
    pub(crate) scopes_supported: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DynamicClientRegistrationRequest {
    pub(crate) redirect_uris: Option<Vec<String>>,
    pub(crate) token_endpoint_auth_method: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DynamicClientRegistrationResponse {
    pub(crate) client_id: String,
    pub(crate) client_id_issued_at: u64,
    pub(crate) redirect_uris: Vec<String>,
    pub(crate) token_endpoint_auth_method: String,
    pub(crate) grant_types: Vec<String>,
    pub(crate) response_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AuthorizeParams {
    pub(crate) response_type: String,
    pub(crate) client_id: String,
    pub(crate) redirect_uri: Option<String>,
    pub(crate) scope: Option<String>,
    pub(crate) state: Option<String>,
    pub(crate) code_challenge: Option<String>,
    pub(crate) code_challenge_method: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthorizeErrorResponse {
    pub(crate) error: String,
    pub(crate) error_description: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenRequest {
    pub(crate) grant_type: String,
    pub(crate) code: Option<String>,
    pub(crate) redirect_uri: Option<String>,
    pub(crate) client_id: Option<String>,
    pub(crate) code_verifier: Option<String>,
    pub(crate) refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OAuthTokenResponse {
    pub(crate) access_token: String,
    pub(crate) token_type: String,
    pub(crate) expires_in: u64,
    pub(crate) refresh_token: Option<String>,
    pub(crate) scope: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct TokenErrorResponse {
    pub(crate) error: String,
    pub(crate) error_description: String,
}
