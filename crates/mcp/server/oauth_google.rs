#[path = "oauth_google/config.rs"]
mod config;
#[path = "oauth_google/handlers_broker.rs"]
mod handlers_broker;
#[path = "oauth_google/handlers_google.rs"]
mod handlers_google;
#[path = "oauth_google/handlers_protected.rs"]
mod handlers_protected;
#[path = "oauth_google/helpers.rs"]
mod helpers;
#[path = "oauth_google/state.rs"]
mod state;
#[path = "oauth_google/tests.rs"]
mod tests;
#[path = "oauth_google/types.rs"]
mod types;

pub(super) use handlers_broker::{oauth_authorize, oauth_register_client};
pub(super) use handlers_google::{
    oauth_authorization_server_metadata, oauth_google_callback, oauth_google_login,
    oauth_google_logout, oauth_google_status, oauth_google_token,
    oauth_protected_resource_metadata,
};
pub(super) use handlers_protected::{oauth_token, require_google_auth};
pub(super) use types::GoogleOAuthState;
