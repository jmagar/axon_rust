use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use redis::AsyncCommands;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::helpers::unix_now_secs;
use super::types::{
    AccessTokenRecord, AuthCodeRecord, GoogleOAuthConfig, GoogleOAuthInner, GoogleOAuthState,
    GoogleTokenResponse, OAUTH_SESSION_TTL_SECS, OAuthError, PendingStateRecord, RateLimitRecord,
    RefreshTokenRecord, RegisteredClient,
};

impl GoogleOAuthState {
    pub(crate) fn from_env(mcp_host: &str, mcp_port: u16) -> Self {
        let config = GoogleOAuthConfig::from_env(mcp_host, mcp_port);
        let redis_client = std::env::var("GOOGLE_OAUTH_REDIS_URL")
            .ok()
            .or_else(|| std::env::var("AXON_REDIS_URL").ok())
            .and_then(|url| redis::Client::open(url).ok());

        Self {
            inner: std::sync::Arc::new(GoogleOAuthInner {
                config,
                http_client: reqwest::Client::new(),
                redis_client,
                pending_state: Mutex::new(HashMap::new()),
                oauth_sessions: Mutex::new(HashMap::new()),
                oauth_clients: Mutex::new(HashMap::new()),
                auth_codes: Mutex::new(HashMap::new()),
                access_tokens: Mutex::new(HashMap::new()),
                refresh_tokens: Mutex::new(HashMap::new()),
                rate_limits: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn configured(&self) -> bool {
        self.inner.config.is_some()
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn config(&self) -> Result<&GoogleOAuthConfig, Response> {
        self.inner.config.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_IMPLEMENTED,
                Json(OAuthError {
                    error: "google oauth not configured",
                }),
            )
                .into_response()
        })
    }

    pub(crate) fn key(&self, suffix: &str) -> String {
        let prefix = self
            .inner
            .config
            .as_ref()
            .map(|c| c.redis_key_prefix.as_str())
            .unwrap_or("axon:mcp:oauth");
        format!("{prefix}:{suffix}")
    }

    pub(crate) async fn redis_conn(&self) -> Option<redis::aio::MultiplexedConnection> {
        let client = self.inner.redis_client.as_ref()?;
        client.get_multiplexed_async_connection().await.ok()
    }

    pub(crate) async fn redis_set_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_secs: Option<u64>,
    ) {
        let Some(mut conn) = self.redis_conn().await else {
            return;
        };
        let Ok(payload) = serde_json::to_string(value) else {
            return;
        };
        if let Some(ttl) = ttl_secs {
            let _: redis::RedisResult<()> = conn.set_ex(key, payload, ttl).await;
        } else {
            let _: redis::RedisResult<()> = conn.set(key, payload).await;
        }
    }

    pub(crate) async fn redis_get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut conn = self.redis_conn().await?;
        let payload: Option<String> = conn.get(key).await.ok()?;
        payload.and_then(|raw| serde_json::from_str::<T>(&raw).ok())
    }

    pub(crate) async fn redis_set_string(&self, key: &str, value: &str, ttl_secs: Option<u64>) {
        let Some(mut conn) = self.redis_conn().await else {
            return;
        };
        if let Some(ttl) = ttl_secs {
            let _: redis::RedisResult<()> = conn.set_ex(key, value, ttl).await;
        } else {
            let _: redis::RedisResult<()> = conn.set(key, value).await;
        }
    }

    pub(crate) async fn redis_get_string(&self, key: &str) -> Option<String> {
        let mut conn = self.redis_conn().await?;
        conn.get(key).await.ok().flatten()
    }

    pub(crate) async fn redis_del(&self, key: &str) {
        let Some(mut conn) = self.redis_conn().await else {
            return;
        };
        let _: redis::RedisResult<usize> = conn.del(key).await;
    }

    pub(crate) async fn get_session_token(&self, session_id: &str) -> Option<GoogleTokenResponse> {
        if let Some(token) = self
            .redis_get_json::<GoogleTokenResponse>(&self.key(&format!("session:{session_id}")))
            .await
        {
            return Some(token);
        }
        self.inner
            .oauth_sessions
            .lock()
            .await
            .get(session_id)
            .cloned()
    }

    pub(crate) async fn set_session_token(&self, session_id: &str, token: GoogleTokenResponse) {
        self.inner
            .oauth_sessions
            .lock()
            .await
            .insert(session_id.to_string(), token.clone());
        self.redis_set_json(
            &self.key(&format!("session:{session_id}")),
            &token,
            Some(OAUTH_SESSION_TTL_SECS),
        )
        .await;
    }

    pub(crate) async fn clear_session_token(&self, session_id: &str) {
        self.inner.oauth_sessions.lock().await.remove(session_id);
        self.redis_del(&self.key(&format!("session:{session_id}")))
            .await;
    }

    pub(crate) async fn is_authenticated(&self, session_id: &str) -> bool {
        self.get_session_token(session_id).await.is_some()
    }

    pub(crate) async fn put_pending_state(&self, state: &str, return_to: &str) {
        let record = PendingStateRecord {
            return_to: return_to.to_string(),
            expires_at_unix: unix_now_secs() + 900,
        };
        self.inner
            .pending_state
            .lock()
            .await
            .insert(state.to_string(), record.clone());
        self.redis_set_string(
            &self.key(&format!("pending_state:{state}")),
            return_to,
            Some(900),
        )
        .await;
    }

    pub(crate) async fn take_pending_state(&self, state: &str) -> Option<String> {
        let key = self.key(&format!("pending_state:{state}"));
        if let Some(v) = self.redis_get_string(&key).await {
            self.redis_del(&key).await;
            return Some(v);
        }
        let record = self.inner.pending_state.lock().await.remove(state)?;
        if unix_now_secs() > record.expires_at_unix {
            return None;
        }
        Some(record.return_to)
    }

    pub(crate) async fn put_client(&self, client_id: &str, client: &RegisteredClient) {
        self.inner
            .oauth_clients
            .lock()
            .await
            .insert(client_id.to_string(), client.clone());
        self.redis_set_json(&self.key(&format!("client:{client_id}")), client, None)
            .await;
    }

    pub(crate) async fn get_client(&self, client_id: &str) -> Option<RegisteredClient> {
        if let Some(client) = self
            .redis_get_json::<RegisteredClient>(&self.key(&format!("client:{client_id}")))
            .await
        {
            return Some(client);
        }
        self.inner
            .oauth_clients
            .lock()
            .await
            .get(client_id)
            .cloned()
    }

    pub(crate) async fn put_auth_code(&self, code: &str, record: &AuthCodeRecord) {
        self.inner
            .auth_codes
            .lock()
            .await
            .insert(code.to_string(), record.clone());
        self.redis_set_json(&self.key(&format!("auth_code:{code}")), record, Some(600))
            .await;
    }

    pub(crate) async fn consume_auth_code(&self, code: &str) -> Option<AuthCodeRecord> {
        let key = self.key(&format!("auth_code:{code}"));
        if let Some(record) = self.redis_get_json::<AuthCodeRecord>(&key).await {
            self.redis_del(&key).await;
            return Some(record);
        }
        self.inner.auth_codes.lock().await.remove(code)
    }

    pub(crate) async fn put_access_token(
        &self,
        token: &str,
        record: &AccessTokenRecord,
        ttl_secs: u64,
    ) {
        self.inner
            .access_tokens
            .lock()
            .await
            .insert(token.to_string(), record.clone());
        self.redis_set_json(
            &self.key(&format!("access_token:{token}")),
            record,
            Some(ttl_secs),
        )
        .await;
    }

    pub(crate) async fn get_access_token(&self, token: &str) -> Option<AccessTokenRecord> {
        if let Some(record) = self
            .redis_get_json::<AccessTokenRecord>(&self.key(&format!("access_token:{token}")))
            .await
        {
            return Some(record);
        }
        self.inner.access_tokens.lock().await.get(token).cloned()
    }

    pub(crate) async fn put_refresh_token(
        &self,
        token: &str,
        record: &RefreshTokenRecord,
        ttl_secs: u64,
    ) {
        self.inner
            .refresh_tokens
            .lock()
            .await
            .insert(token.to_string(), record.clone());
        self.redis_set_json(
            &self.key(&format!("refresh_token:{token}")),
            record,
            Some(ttl_secs),
        )
        .await;
    }

    pub(crate) async fn get_refresh_token(&self, token: &str) -> Option<RefreshTokenRecord> {
        if let Some(record) = self
            .redis_get_json::<RefreshTokenRecord>(&self.key(&format!("refresh_token:{token}")))
            .await
        {
            return Some(record);
        }
        self.inner.refresh_tokens.lock().await.get(token).cloned()
    }

    pub(crate) async fn delete_refresh_token(&self, token: &str) {
        self.inner.refresh_tokens.lock().await.remove(token);
        self.redis_del(&self.key(&format!("refresh_token:{token}")))
            .await;
    }

    pub(crate) async fn cleanup_expired_in_memory(&self) {
        let now = unix_now_secs();

        let pending_evicted = {
            let mut map = self.inner.pending_state.lock().await;
            let before = map.len();
            map.retain(|_, rec| rec.expires_at_unix > now);
            before - map.len()
        };

        let auth_evicted = {
            let mut map = self.inner.auth_codes.lock().await;
            let before = map.len();
            map.retain(|_, rec| rec.expires_at_unix > now);
            before - map.len()
        };

        let access_evicted = {
            let mut map = self.inner.access_tokens.lock().await;
            let before = map.len();
            map.retain(|_, rec| rec.expires_at_unix > now);
            before - map.len()
        };

        let refresh_evicted = {
            let mut map = self.inner.refresh_tokens.lock().await;
            let before = map.len();
            map.retain(|_, rec| rec.expires_at_unix > now);
            before - map.len()
        };

        let rl_evicted = {
            let mut map = self.inner.rate_limits.lock().await;
            let before = map.len();
            map.retain(|_, rec| rec.reset_at_unix > now);
            before - map.len()
        };

        let evicted =
            pending_evicted + auth_evicted + access_evicted + refresh_evicted + rl_evicted;
        if evicted > 0 {
            info!(
                target: "axon.mcp.oauth",
                evicted,
                "evicted expired oauth records from in-memory TTL stores"
            );
        }
    }

    pub(crate) async fn check_rate_limit(
        &self,
        bucket: &str,
        limit: u64,
        window_secs: u64,
    ) -> Result<(), Response> {
        let now = unix_now_secs();
        let key = self.key(&format!("ratelimit:{bucket}"));

        if let Some(mut conn) = self.redis_conn().await {
            let script = redis::Script::new(
                r"
                local c = redis.call('INCR', KEYS[1])
                if c == 1 then
                    redis.call('EXPIRE', KEYS[1], ARGV[1])
                end
                return c
                ",
            );
            let count: u64 = script
                .key(&key)
                .arg(window_secs as i64)
                .invoke_async(&mut conn)
                .await
                .unwrap_or(0);
            if count > limit {
                warn!(target: "axon.mcp.oauth", bucket, count, limit, "rate limit exceeded (redis)");
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "error": "rate_limited",
                        "error_description": "too many requests",
                        "retry_after_seconds": window_secs
                    })),
                )
                    .into_response());
            }
            return Ok(());
        }

        let mut rl = self.inner.rate_limits.lock().await;
        let entry = rl.entry(bucket.to_string()).or_insert(RateLimitRecord {
            count: 0,
            reset_at_unix: now + window_secs,
        });
        if now >= entry.reset_at_unix {
            entry.count = 0;
            entry.reset_at_unix = now + window_secs;
        }
        entry.count += 1;
        if entry.count > limit {
            warn!(target: "axon.mcp.oauth", bucket, count = entry.count, limit, "rate limit exceeded (memory)");
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "error_description": "too many requests",
                    "retry_after_seconds": entry.reset_at_unix.saturating_sub(now)
                })),
            )
                .into_response());
        }
        Ok(())
    }
}
