use axum::{
    Form, Json,
    extract::State,
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::Engine;
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

use super::helpers::{
    is_allowed_redirect_uri, normalize_loopback_redirect_uri, request_identity_from_headers,
    required_scopes, token_error_response, unauthorized_response, unix_now_secs,
};
use super::types::{
    AccessTokenRecord, GoogleOAuthState, OAUTH_REFRESH_TTL_SECS, OAuthTokenResponse,
    RefreshTokenRecord, TokenRequest,
};

pub(crate) async fn oauth_token(
    State(state): State<GoogleOAuthState>,
    headers: axum::http::HeaderMap,
    Form(form): Form<TokenRequest>,
) -> Response {
    state.cleanup_expired_in_memory().await;
    let identity = request_identity_from_headers(&headers);
    if let Err(resp) = state
        .check_rate_limit(&format!("token:{identity}"), 120, 60)
        .await
    {
        return resp;
    }
    let cfg = state.config().ok();

    let grant = form.grant_type.as_str();
    if grant == "authorization_code" {
        let client_id = match form.client_id {
            Some(value) => value,
            None => {
                return token_error_response(
                    "invalid_request",
                    "client_id is required",
                    StatusCode::BAD_REQUEST,
                );
            }
        };
        let code = match form.code {
            Some(value) => value,
            None => {
                return token_error_response(
                    "invalid_request",
                    "code is required",
                    StatusCode::BAD_REQUEST,
                );
            }
        };
        let redirect_uri = match form.redirect_uri {
            Some(value) => value,
            None => {
                return token_error_response(
                    "invalid_request",
                    "redirect_uri is required",
                    StatusCode::BAD_REQUEST,
                );
            }
        };
        let redirect_uri = match normalize_loopback_redirect_uri(&redirect_uri) {
            Some(uri) => uri,
            None => {
                return token_error_response(
                    "invalid_request",
                    "redirect_uri is invalid",
                    StatusCode::BAD_REQUEST,
                );
            }
        };
        if let Some(cfg) = cfg
            && !is_allowed_redirect_uri(&redirect_uri, cfg.redirect_policy)
        {
            return token_error_response(
                "invalid_request",
                "redirect_uri violates server redirect policy",
                StatusCode::BAD_REQUEST,
            );
        }

        let record = state.consume_auth_code(&code).await;

        let record = match record {
            Some(value) => value,
            None => {
                return token_error_response(
                    "invalid_grant",
                    "invalid or expired authorization code",
                    StatusCode::BAD_REQUEST,
                );
            }
        };

        if unix_now_secs() > record.expires_at_unix {
            return token_error_response(
                "invalid_grant",
                "authorization code expired",
                StatusCode::BAD_REQUEST,
            );
        }

        if record.client_id != client_id || record.redirect_uri != redirect_uri {
            return token_error_response(
                "invalid_grant",
                "authorization code does not match client_id/redirect_uri",
                StatusCode::BAD_REQUEST,
            );
        }

        if let Some(challenge) = record.code_challenge {
            let verifier = match form.code_verifier {
                Some(value) => value,
                None => {
                    return token_error_response(
                        "invalid_request",
                        "code_verifier is required for PKCE",
                        StatusCode::BAD_REQUEST,
                    );
                }
            };

            let method = record
                .code_challenge_method
                .unwrap_or_else(|| "S256".to_string());
            if method != "S256" {
                return token_error_response(
                    "invalid_request",
                    "code_challenge_method must be S256",
                    StatusCode::BAD_REQUEST,
                );
            }
            let digest = Sha256::digest(verifier.as_bytes());
            let computed = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);

            if computed != challenge {
                return token_error_response(
                    "invalid_grant",
                    "invalid code_verifier",
                    StatusCode::BAD_REQUEST,
                );
            }
        }

        let access_token = format!("atk_{}", Uuid::new_v4());
        let refresh_token = format!("rtk_{}", Uuid::new_v4());
        let expires_in = 3600_u64;

        let access_record = AccessTokenRecord {
            scope: record.scope.clone(),
            expires_at_unix: unix_now_secs() + expires_in,
        };
        state
            .put_access_token(&access_token, &access_record, expires_in)
            .await;

        let refresh_record = RefreshTokenRecord {
            client_id,
            scope: record.scope.clone(),
            expires_at_unix: unix_now_secs() + OAUTH_REFRESH_TTL_SECS,
        };
        state
            .put_refresh_token(&refresh_token, &refresh_record, OAUTH_REFRESH_TTL_SECS)
            .await;
        info!(
            target: "axon.mcp.oauth",
            identity,
            "token exchange succeeded (authorization_code)"
        );

        return (
            StatusCode::OK,
            Json(OAuthTokenResponse {
                access_token,
                token_type: "Bearer".to_string(),
                expires_in,
                refresh_token: Some(refresh_token),
                scope: record.scope,
            }),
        )
            .into_response();
    }

    if grant == "refresh_token" {
        let client_id = match form.client_id {
            Some(value) => value,
            None => {
                return token_error_response(
                    "invalid_request",
                    "client_id is required",
                    StatusCode::BAD_REQUEST,
                );
            }
        };

        let refresh = match form.refresh_token {
            Some(value) => value,
            None => {
                return token_error_response(
                    "invalid_request",
                    "refresh_token is required",
                    StatusCode::BAD_REQUEST,
                );
            }
        };

        let refresh_record = state.get_refresh_token(&refresh).await;

        let refresh_record = match refresh_record {
            Some(value) => value,
            None => {
                return token_error_response(
                    "invalid_grant",
                    "invalid refresh_token",
                    StatusCode::BAD_REQUEST,
                );
            }
        };

        if refresh_record.client_id != client_id {
            return token_error_response(
                "invalid_grant",
                "refresh_token does not belong to this client",
                StatusCode::BAD_REQUEST,
            );
        }
        if unix_now_secs() > refresh_record.expires_at_unix {
            return token_error_response(
                "invalid_grant",
                "refresh_token expired",
                StatusCode::BAD_REQUEST,
            );
        }

        let access_token = format!("atk_{}", Uuid::new_v4());
        let new_refresh_token = format!("rtk_{}", Uuid::new_v4());
        let expires_in = 3600_u64;
        let access_record = AccessTokenRecord {
            scope: refresh_record.scope.clone(),
            expires_at_unix: unix_now_secs() + expires_in,
        };
        state
            .put_access_token(&access_token, &access_record, expires_in)
            .await;
        state.delete_refresh_token(&refresh).await;
        let rotated_refresh = RefreshTokenRecord {
            client_id,
            scope: refresh_record.scope.clone(),
            expires_at_unix: unix_now_secs() + OAUTH_REFRESH_TTL_SECS,
        };
        state
            .put_refresh_token(&new_refresh_token, &rotated_refresh, OAUTH_REFRESH_TTL_SECS)
            .await;
        info!(
            target: "axon.mcp.oauth",
            identity,
            "token exchange succeeded (refresh_token)"
        );

        return (
            StatusCode::OK,
            Json(OAuthTokenResponse {
                access_token,
                token_type: "Bearer".to_string(),
                expires_in,
                refresh_token: Some(new_refresh_token),
                scope: refresh_record.scope,
            }),
        )
            .into_response();
    }

    token_error_response(
        "unsupported_grant_type",
        "supported grant_type values are authorization_code and refresh_token",
        StatusCode::BAD_REQUEST,
    )
}

pub(crate) async fn require_google_auth(
    State(state): State<GoogleOAuthState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if !req.uri().path().starts_with("/mcp") {
        return next.run(req).await;
    }

    if !state.configured() {
        return unauthorized_response(
            &state,
            serde_json::json!({
                "error": "google oauth is not configured on this server"
            }),
        );
    }

    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION)
        && let Ok(auth_value) = auth_header.to_str()
        && let Some(token) = auth_value.strip_prefix("Bearer ")
    {
        let record = state.get_access_token(token).await;

        if let Some(record) = record {
            if unix_now_secs() <= record.expires_at_unix {
                let token_scopes = record
                    .scope
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect::<std::collections::HashSet<String>>();
                let needed_scopes = required_scopes(&state);
                if needed_scopes
                    .iter()
                    .all(|scope| token_scopes.contains(scope))
                {
                    return next.run(req).await;
                }
                return unauthorized_response(
                    &state,
                    serde_json::json!({
                        "error": "insufficient_scope",
                        "required_scopes": needed_scopes,
                    }),
                );
            }
        }

        return unauthorized_response(
            &state,
            serde_json::json!({
                "error": "invalid_token"
            }),
        );
    }

    unauthorized_response(
        &state,
        serde_json::json!({
            "error": "authorization_required"
        }),
    )
}
