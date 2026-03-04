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
    OAuthHtmlPageConfig, build_session_clear_cookie, build_session_set_cookie,
    extract_cookie_value, oauth_html_page, request_identity,
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
    let identity = request_identity(&req);
    state
        .check_rate_limit(&format!("login:{identity}"), 30, 60)
        .await?;

    let csrf_state = Uuid::new_v4().to_string();
    let return_to = query.return_to.unwrap_or_else(|| "/".to_string());
    let return_to = if return_to.starts_with('/') && !return_to.starts_with("//") {
        return_to
    } else {
        "/".to_string()
    };
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

async fn exchange_google_code(
    http_client: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<GoogleTokenResponse, Response> {
    http_client
        .post(token_url)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| {
            oauth_html_page(OAuthHtmlPageConfig {
                status: StatusCode::BAD_GATEWAY,
                title: "Failed to reach Google's token endpoint.",
                subtitle: "Upstream Error",
                detail: &format!("token request failed: {e}"),
                primary: ("/oauth/google/login", "Try Again"),
                secondary: ("/mcp", "Go to MCP Endpoint"),
            })
        })?
        .error_for_status()
        .map_err(|e| {
            oauth_html_page(OAuthHtmlPageConfig {
                status: StatusCode::BAD_GATEWAY,
                title: "Google rejected the token exchange.",
                subtitle: "Token Exchange Failed",
                detail: &format!("token exchange failed: {e}"),
                primary: ("/oauth/google/login", "Try Again"),
                secondary: ("/mcp", "Go to MCP Endpoint"),
            })
        })?
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| {
            oauth_html_page(OAuthHtmlPageConfig {
                status: StatusCode::BAD_GATEWAY,
                title: "Google returned an unreadable token payload.",
                subtitle: "Invalid Token Response",
                detail: &format!("invalid token response: {e}"),
                primary: ("/oauth/google/login", "Try Again"),
                secondary: ("/mcp", "Go to MCP Endpoint"),
            })
        })
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
        return Err(oauth_html_page(OAuthHtmlPageConfig {
            status: StatusCode::UNAUTHORIZED,
            title: "Google authorization did not complete.",
            subtitle: "Sign-in Failed",
            detail: &format!("Google returned: {error}"),
            primary: ("/oauth/google/login", "Try Again"),
            secondary: ("/mcp", "Go to MCP Endpoint"),
        }));
    }

    let received_state = params.state.ok_or_else(|| {
        oauth_html_page(OAuthHtmlPageConfig {
            status: StatusCode::BAD_REQUEST,
            title: "Missing OAuth state parameter.",
            subtitle: "Invalid Request",
            detail: "The callback request did not include a valid state value.",
            primary: ("/oauth/google/login", "Start Login"),
            secondary: ("/mcp", "Go to MCP Endpoint"),
        })
    })?;
    let return_to = state
        .take_pending_state(&received_state)
        .await
        .ok_or_else(|| {
            oauth_html_page(OAuthHtmlPageConfig {
                status: StatusCode::UNAUTHORIZED,
                title: "No pending OAuth state found.",
                subtitle: "Session Expired",
                detail: "Your login session likely expired. Start authentication again.",
                primary: ("/oauth/google/login", "Start Login"),
                secondary: ("/mcp", "Go to MCP Endpoint"),
            })
        })?;

    let code = params.code.ok_or_else(|| {
        oauth_html_page(OAuthHtmlPageConfig {
            status: StatusCode::BAD_REQUEST,
            title: "Missing authorization code.",
            subtitle: "Invalid Callback",
            detail: "Google did not provide an authorization code in the callback.",
            primary: ("/oauth/google/login", "Start Login"),
            secondary: ("/mcp", "Go to MCP Endpoint"),
        })
    })?;

    let token = exchange_google_code(
        &state.inner.http_client,
        &cfg.token_url,
        &cfg.client_id,
        &cfg.client_secret,
        &code,
        &cfg.redirect_uri,
    )
    .await?;

    let session_id = Uuid::new_v4().to_string();
    state.set_session_token(&session_id, token).await;
    info!(
        target: "axon.mcp.oauth",
        identity,
        "oauth callback exchange succeeded"
    );

    if return_to == "/" {
        let mut response = oauth_html_page(OAuthHtmlPageConfig {
            status: StatusCode::OK,
            title: "OAuth token was stored in Redis and memory.",
            subtitle: "Google Login Successful",
            detail: "Authentication completed. You can close this tab and return to your MCP client.",
            primary: ("/mcp", "Open MCP Endpoint"),
            secondary: ("/oauth/google/status", "View Auth Status"),
        });
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
    let mut response = oauth_html_page(OAuthHtmlPageConfig {
        status: StatusCode::OK,
        title: "OAuth token has been cleared.",
        subtitle: "Logged Out",
        detail: "Google session token removed from Redis and in-memory state.",
        primary: ("/oauth/google/login", "Sign In Again"),
        secondary: ("/mcp", "Go to MCP Endpoint"),
    });
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
