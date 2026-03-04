use super::types::{GoogleOAuthConfig, RedirectPolicy};

impl GoogleOAuthConfig {
    pub(super) fn from_env(mcp_host: &str, mcp_port: u16) -> Option<Self> {
        let client_id = std::env::var("GOOGLE_OAUTH_CLIENT_ID").ok()?;
        let client_secret = std::env::var("GOOGLE_OAUTH_CLIENT_SECRET").ok()?;

        if client_id.trim().is_empty() || client_secret.trim().is_empty() {
            return None;
        }

        let auth_url = std::env::var("GOOGLE_OAUTH_AUTH_URL")
            .unwrap_or_else(|_| "https://accounts.google.com/o/oauth2/v2/auth".to_string());
        let token_url = std::env::var("GOOGLE_OAUTH_TOKEN_URL")
            .unwrap_or_else(|_| "https://oauth2.googleapis.com/token".to_string());
        let redirect_path = std::env::var("GOOGLE_OAUTH_REDIRECT_PATH")
            .unwrap_or_else(|_| "/oauth/google/callback".to_string());
        let redirect_path = if redirect_path.starts_with('/') {
            redirect_path
        } else {
            format!("/{redirect_path}")
        };
        let redirect_host = std::env::var("GOOGLE_OAUTH_REDIRECT_HOST").unwrap_or_else(|_| {
            if mcp_host == "0.0.0.0" {
                "localhost".to_string()
            } else {
                mcp_host.to_string()
            }
        });

        let broker_issuer = std::env::var("GOOGLE_OAUTH_BROKER_ISSUER")
            .unwrap_or_else(|_| format!("http://{redirect_host}:{mcp_port}"))
            .trim_end_matches('/')
            .to_string();

        let redirect_uri = std::env::var("GOOGLE_OAUTH_REDIRECT_URI")
            .unwrap_or_else(|_| format!("http://{redirect_host}:{mcp_port}{redirect_path}"));
        let resource_server_url = format!("{broker_issuer}/mcp");
        let resource_metadata_url = format!("{broker_issuer}/.well-known/oauth-protected-resource");
        let authorization_endpoint = format!("{broker_issuer}/oauth/authorize");
        let token_endpoint = format!("{broker_issuer}/oauth/token");
        let registration_endpoint = format!("{broker_issuer}/oauth/register");
        let redis_key_prefix = std::env::var("GOOGLE_OAUTH_REDIS_PREFIX")
            .unwrap_or_else(|_| "axon:mcp:oauth".to_string());

        let scopes = std::env::var("GOOGLE_OAUTH_SCOPES")
            .unwrap_or_else(|_| "openid email profile".to_string())
            .split_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let dcr_token = std::env::var("GOOGLE_OAUTH_DCR_TOKEN")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let redirect_policy = match std::env::var("GOOGLE_OAUTH_REDIRECT_POLICY")
            .unwrap_or_else(|_| "loopback_only".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "any" => RedirectPolicy::Any,
            _ => RedirectPolicy::LoopbackOnly,
        };

        Some(Self {
            auth_url,
            token_url,
            broker_issuer,
            authorization_endpoint,
            token_endpoint,
            registration_endpoint,
            client_id,
            client_secret,
            redirect_uri,
            resource_server_url,
            resource_metadata_url,
            redis_key_prefix,
            scopes,
            dcr_token,
            redirect_policy,
        })
    }
}
