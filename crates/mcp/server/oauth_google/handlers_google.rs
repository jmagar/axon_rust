use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use reqwest::Url;
use tracing::info;
use uuid::Uuid;

use super::helpers::{
    build_session_clear_cookie, build_session_set_cookie, extract_cookie_value, oauth_html_page,
    request_identity,
};
use super::types::{
    AuthorizationServerMetadata, CallbackParams, GoogleOAuthState, GoogleTokenResponse, LoginQuery,
    OAUTH_SESSION_COOKIE, OAuthError, OAuthStatus, ProtectedResourceMetadata,
};

pub(crate) async fn oauth_google_status(
    State(state): State<GoogleOAuthState>,
    req: axum::extract::Request,
) -> Result<Json<OAuthStatus>, Response> {
    let authenticated = if let Some(session_id) = extract_cookie_value(&req, OAUTH_SESSION_COOKIE) {
        state.is_authenticated(&session_id).await
    } else {
        false
    };

    Ok(Json(match state.inner.config.as_ref() {
        Some(cfg) => OAuthStatus {
            configured: true,
            authenticated,
            redirect_uri: Some(cfg.redirect_uri.clone()),
            scopes: cfg.scopes.clone(),
        },
        None => OAuthStatus {
            configured: false,
            authenticated: false,
            redirect_uri: None,
            scopes: vec![],
        },
    }))
}

#[allow(clippy::result_large_err)]
pub(crate) async fn oauth_google_login(
    State(state): State<GoogleOAuthState>,
    Query(query): Query<LoginQuery>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, Response> {
    let cfg = state.config()?;
    state.cleanup_expired_in_memory().await;
    let identity = request_identity(&req);
    state
        .check_rate_limit(&format!("login:{identity}"), 30, 60)
        .await?;

    let csrf_state = Uuid::new_v4().to_string();
    let return_to = query.return_to.unwrap_or_else(|| "/".to_string());
    state.put_pending_state(&csrf_state, &return_to).await;

    let mut auth_url = Url::parse(&cfg.auth_url).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(OAuthError {
                error: "invalid google auth url",
            }),
        )
            .into_response()
    })?;

    let scope_value = cfg.scopes.join(" ");
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", &cfg.client_id)
        .append_pair("redirect_uri", &cfg.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &scope_value)
        .append_pair("access_type", "offline")
        .append_pair("state", &csrf_state);

    info!(
        target: "axon.mcp.oauth",
        identity,
        "oauth login initiated"
    );

    Ok(Redirect::temporary(auth_url.as_str()))
}

#[allow(clippy::result_large_err)]
pub(crate) async fn oauth_google_callback(
    State(state): State<GoogleOAuthState>,
    Query(params): Query<CallbackParams>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, Response> {
    let cfg = state.config()?;
    let identity = request_identity(&req);
    state
        .check_rate_limit(&format!("callback:{identity}"), 60, 60)
        .await?;

    if let Some(error) = params.error {
        return Err(oauth_html_page(
            StatusCode::UNAUTHORIZED,
            "Google authorization did not complete.",
            "Sign-in Failed",
            &format!("Google returned: {error}"),
            "/oauth/google/login",
            "Try Again",
            "/mcp",
            "Go to MCP Endpoint",
        ));
    }

    let received_state = params.state.ok_or_else(|| {
        oauth_html_page(
            StatusCode::BAD_REQUEST,
            "Missing OAuth state parameter.",
            "Invalid Request",
            "The callback request did not include a valid state value.",
            "/oauth/google/login",
            "Start Login",
            "/mcp",
            "Go to MCP Endpoint",
        )
    })?;
    let return_to = state
        .take_pending_state(&received_state)
        .await
        .ok_or_else(|| {
            oauth_html_page(
                StatusCode::UNAUTHORIZED,
                "No pending OAuth state found.",
                "Session Expired",
                "Your login session likely expired. Start authentication again.",
                "/oauth/google/login",
                "Start Login",
                "/mcp",
                "Go to MCP Endpoint",
            )
        })?;

    let code = params.code.ok_or_else(|| {
        oauth_html_page(
            StatusCode::BAD_REQUEST,
            "Missing authorization code.",
            "Invalid Callback",
            "Google did not provide an authorization code in the callback.",
            "/oauth/google/login",
            "Start Login",
            "/mcp",
            "Go to MCP Endpoint",
        )
    })?;

    let token = state
        .inner
        .http_client
        .post(&cfg.token_url)
        .form(&[
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("code", code.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", cfg.redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| {
            oauth_html_page(
                StatusCode::BAD_GATEWAY,
                "Failed to reach Google's token endpoint.",
                "Upstream Error",
                &format!("token request failed: {e}"),
                "/oauth/google/login",
                "Try Again",
                "/mcp",
                "Go to MCP Endpoint",
            )
        })?
        .error_for_status()
        .map_err(|e| {
            oauth_html_page(
                StatusCode::BAD_GATEWAY,
                "Google rejected the token exchange.",
                "Token Exchange Failed",
                &format!("token exchange failed: {e}"),
                "/oauth/google/login",
                "Try Again",
                "/mcp",
                "Go to MCP Endpoint",
            )
        })?
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| {
            oauth_html_page(
                StatusCode::BAD_GATEWAY,
                "Google returned an unreadable token payload.",
                "Invalid Token Response",
                &format!("invalid token response: {e}"),
                "/oauth/google/login",
                "Try Again",
                "/mcp",
                "Go to MCP Endpoint",
            )
        })?;

    let session_id = Uuid::new_v4().to_string();
    state.set_session_token(&session_id, token).await;
    info!(
        target: "axon.mcp.oauth",
        identity,
        "oauth callback exchange succeeded"
    );

    if return_to == "/" {
        let mut response = oauth_html_page(
            StatusCode::OK,
            "OAuth token was stored in Redis and memory.",
            "Google Login Successful",
            "Authentication completed. You can close this tab and return to your MCP client.",
            "/mcp",
            "Open MCP Endpoint",
            "/oauth/google/status",
            "View Auth Status",
        );
        if let Some(cookie) = build_session_set_cookie(&state, &session_id) {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
        Ok(response)
    } else {
        let mut response = Redirect::temporary(&return_to).into_response();
        if let Some(cookie) = build_session_set_cookie(&state, &session_id) {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
        Ok(response)
    }
}

#[allow(clippy::result_large_err)]
pub(crate) async fn oauth_google_token(
    State(state): State<GoogleOAuthState>,
    req: axum::extract::Request,
) -> Result<Json<GoogleTokenResponse>, Response> {
    let session_id = extract_cookie_value(&req, OAUTH_SESSION_COOKIE)
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing oauth session").into_response())?;
    let token = state
        .get_session_token(&session_id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, "no token stored").into_response())?;
    Ok(Json(token))
}

#[allow(clippy::result_large_err)]
pub(crate) async fn oauth_google_logout(
    State(state): State<GoogleOAuthState>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, Response> {
    if let Some(session_id) = extract_cookie_value(&req, OAUTH_SESSION_COOKIE) {
        state.clear_session_token(&session_id).await;
    }
    let mut response = oauth_html_page(
        StatusCode::OK,
        "OAuth token has been cleared.",
        "Logged Out",
        "Google session token removed from Redis and in-memory state.",
        "/oauth/google/login",
        "Sign In Again",
        "/mcp",
        "Go to MCP Endpoint",
    );
    if let Some(cookie) = build_session_clear_cookie(&state) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    Ok(response)
}

#[allow(clippy::result_large_err)]
pub(crate) async fn oauth_protected_resource_metadata(
    State(state): State<GoogleOAuthState>,
) -> Result<Json<ProtectedResourceMetadata>, Response> {
    let cfg = state.config()?;
    Ok(Json(ProtectedResourceMetadata {
        resource: cfg.resource_server_url.clone(),
        authorization_servers: vec![cfg.broker_issuer.clone()],
        scopes_supported: cfg.scopes.clone(),
    }))
}

#[allow(clippy::result_large_err)]
pub(crate) async fn oauth_authorization_server_metadata(
    State(state): State<GoogleOAuthState>,
) -> Result<Json<AuthorizationServerMetadata>, Response> {
    let cfg = state.config()?;
    Ok(Json(AuthorizationServerMetadata {
        issuer: cfg.broker_issuer.clone(),
        authorization_endpoint: cfg.authorization_endpoint.clone(),
        token_endpoint: cfg.token_endpoint.clone(),
        registration_endpoint: cfg.registration_endpoint.clone(),
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        token_endpoint_auth_methods_supported: vec!["none".to_string()],
        code_challenge_methods_supported: vec!["S256".to_string()],
        scopes_supported: cfg.scopes.clone(),
    }))
}
