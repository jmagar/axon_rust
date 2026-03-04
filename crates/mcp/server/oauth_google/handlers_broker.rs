use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use reqwest::Url;
use tracing::{info, warn};
use uuid::Uuid;

use super::helpers::{
    append_query_pairs, bearer_token_from_headers, extract_cookie_value, is_allowed_redirect_uri,
    normalize_loopback_redirect_uri, request_identity, request_identity_from_headers,
    unix_now_secs,
};
use super::types::{
    AuthCodeRecord, AuthorizeErrorResponse, AuthorizeParams, DynamicClientRegistrationRequest,
    DynamicClientRegistrationResponse, GoogleOAuthState, OAUTH_SESSION_COOKIE, RegisteredClient,
};

pub(crate) async fn oauth_register_client(
    State(state): State<GoogleOAuthState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<DynamicClientRegistrationRequest>,
) -> Result<Json<DynamicClientRegistrationResponse>, Response> {
    let cfg = state.config()?;
    let identity = request_identity_from_headers(&headers);
    state
        .check_rate_limit(&format!("register:{identity}"), 20, 60)
        .await?;

    if let Some(expected) = &cfg.dcr_token {
        let provided = bearer_token_from_headers(&headers);
        if provided.as_deref() != Some(expected.as_str()) {
            warn!(
                target: "axon.mcp.oauth",
                identity,
                "dynamic client registration rejected: missing/invalid token"
            );
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "invalid_client",
                    "error_description": "dynamic registration requires bearer token"
                })),
            )
                .into_response());
        }
    }

    if let Some(method) = payload.token_endpoint_auth_method.as_deref()
        && method != "none"
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_client_metadata",
                "error_description": "only token_endpoint_auth_method=none is supported"
            })),
        )
            .into_response());
    }

    let redirect_uris = payload
        .redirect_uris
        .filter(|uris| !uris.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_redirect_uri",
                    "error_description": "redirect_uris is required"
                })),
            )
                .into_response()
        })?
        .into_iter()
        .filter_map(|uri| normalize_loopback_redirect_uri(&uri))
        .filter(|uri| is_allowed_redirect_uri(uri, cfg.redirect_policy))
        .collect::<Vec<_>>();

    if redirect_uris.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_redirect_uri",
                "error_description": "redirect_uris must include at least one valid URI"
            })),
        )
            .into_response());
    }

    let client_id = format!("axon-{}", Uuid::new_v4());
    let client = RegisteredClient {
        redirect_uris: redirect_uris.clone(),
    };
    state.put_client(&client_id, &client).await?;
    info!(
        target: "axon.mcp.oauth",
        identity,
        client_id,
        redirect_count = client.redirect_uris.len(),
        "dynamic client registered"
    );

    Ok(Json(DynamicClientRegistrationResponse {
        client_id,
        client_id_issued_at: unix_now_secs(),
        redirect_uris,
        token_endpoint_auth_method: "none".to_string(),
        grant_types: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        response_types: vec!["code".to_string()],
    }))
}

// RFC 6749 §4.1.2.1: validate client_id and redirect_uri BEFORE any error
// redirects. Error responses must NOT be sent to an unvalidated redirect_uri.
fn validate_authorize_redirect_uri(
    state: &GoogleOAuthState,
    params: &AuthorizeParams,
    registered: &RegisteredClient,
) -> Result<String, Response> {
    let redirect_uri = match params.redirect_uri.clone() {
        Some(uri) => uri,
        None => {
            if registered.redirect_uris.len() == 1 {
                registered.redirect_uris[0].clone()
            } else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(AuthorizeErrorResponse {
                        error: "invalid_request".to_string(),
                        error_description: "redirect_uri is required".to_string(),
                    }),
                )
                    .into_response());
            }
        }
    };
    let redirect_uri = match normalize_loopback_redirect_uri(&redirect_uri) {
        Some(uri) => uri,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(AuthorizeErrorResponse {
                    error: "invalid_request".to_string(),
                    error_description: "redirect_uri is invalid".to_string(),
                }),
            )
                .into_response());
        }
    };
    let cfg = match state.config() {
        Ok(cfg) => cfg,
        Err(resp) => return Err(resp),
    };
    if !is_allowed_redirect_uri(&redirect_uri, cfg.redirect_policy) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthorizeErrorResponse {
                error: "invalid_request".to_string(),
                error_description: "redirect_uri violates server redirect policy".to_string(),
            }),
        )
            .into_response());
    }
    if !registered
        .redirect_uris
        .iter()
        .any(|uri| uri == &redirect_uri)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthorizeErrorResponse {
                error: "invalid_request".to_string(),
                error_description: "redirect_uri is not registered for this client".to_string(),
            }),
        )
            .into_response());
    }
    Ok(redirect_uri)
}

fn validate_pkce_params(params: &AuthorizeParams) -> Result<(), Response> {
    if params.code_challenge.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthorizeErrorResponse {
                error: "invalid_request".to_string(),
                error_description: "code_challenge is required".to_string(),
            }),
        )
            .into_response());
    }
    if params.code_challenge_method.as_deref() != Some("S256") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthorizeErrorResponse {
                error: "invalid_request".to_string(),
                error_description: "code_challenge_method must be S256".to_string(),
            }),
        )
            .into_response());
    }
    Ok(())
}

fn validate_scope(
    scope_opt: Option<String>,
    allowed_scopes: &[String],
) -> Result<String, Response> {
    let scope = scope_opt.unwrap_or_else(|| "openid email profile".to_string());
    let requested: Vec<&str> = scope.split_whitespace().collect();
    for s in &requested {
        if !allowed_scopes.iter().any(|a| a == s) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(AuthorizeErrorResponse {
                    error: "invalid_scope".to_string(),
                    error_description: format!("scope '{s}' is not allowed"),
                }),
            )
                .into_response());
        }
    }
    Ok(scope)
}

pub(crate) async fn oauth_authorize(
    State(state): State<GoogleOAuthState>,
    Query(params): Query<AuthorizeParams>,
    req: axum::extract::Request,
) -> Response {
    let identity = request_identity(&req);
    if let Err(resp) = state
        .check_rate_limit(&format!("authorize:{identity}"), 60, 60)
        .await
    {
        return resp;
    }

    let session_id = extract_cookie_value(&req, OAUTH_SESSION_COOKIE);
    let authenticated = match session_id.as_deref() {
        Some(id) => state.is_authenticated(id).await,
        None => false,
    };
    if !authenticated {
        let return_to = req.uri().to_string();
        let mut login_url =
            Url::parse("http://localhost/oauth/google/login").unwrap_or_else(|_| {
                Url::parse("http://localhost/").expect("localhost parse must succeed")
            });
        login_url
            .query_pairs_mut()
            .append_pair("return_to", &return_to);
        let redirect = format!(
            "/oauth/google/login?{}",
            login_url.query().unwrap_or_default()
        );
        return Redirect::temporary(&redirect).into_response();
    }

    let registered = match state.get_client(&params.client_id).await {
        Some(client) => client,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthorizeErrorResponse {
                    error: "unauthorized_client".to_string(),
                    error_description: "unknown client_id".to_string(),
                }),
            )
                .into_response();
        }
    };

    let redirect_uri = match validate_authorize_redirect_uri(&state, &params, &registered) {
        Ok(uri) => uri,
        Err(resp) => return resp,
    };

    // redirect_uri is now validated — safe to use for error redirects below
    if params.response_type != "code" {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthorizeErrorResponse {
                error: "unsupported_response_type".to_string(),
                error_description: "only response_type=code is supported".to_string(),
            }),
        )
            .into_response();
    }

    if let Err(resp) = validate_pkce_params(&params) {
        return resp;
    }

    let cfg = match state.config() {
        Ok(cfg) => cfg,
        Err(resp) => return resp,
    };
    let scope = match validate_scope(params.scope, &cfg.scopes) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let auth_code = Uuid::new_v4().to_string();
    let record = AuthCodeRecord {
        client_id: params.client_id,
        redirect_uri: redirect_uri.clone(),
        scope,
        code_challenge: params.code_challenge,
        code_challenge_method: Some("S256".to_string()),
        expires_at_unix: unix_now_secs() + 600,
    };
    if let Err(resp) = state.put_auth_code(&auth_code, &record).await {
        return resp;
    }
    info!(
        target: "axon.mcp.oauth",
        identity,
        client_id = record.client_id,
        "authorization code issued"
    );

    let mut query = vec![("code", auth_code)];
    if let Some(state_param) = params.state {
        query.push(("state", state_param));
    }

    match append_query_pairs(&redirect_uri, &query) {
        Ok(url) => Redirect::temporary(&url).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(AuthorizeErrorResponse {
                error: "invalid_request".to_string(),
                error_description: e,
            }),
        )
            .into_response(),
    }
}
