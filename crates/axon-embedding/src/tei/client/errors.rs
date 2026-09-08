use super::{ApiError, ENDPOINT_MARKER, ErrorStage, StatusCode, TEI_COOLDOWN_SECS, TeiClient};
use axon_error::cooling::ProviderCooling;
use chrono::Utc;
use std::time::Duration;

impl TeiClient {
    pub(super) fn response_error(&self, error: super::response::ResponseError) -> ApiError {
        match error {
            super::response::ResponseError::Transport(error) => {
                self.transport(super::policy::error_category(&error))
            }
            super::response::ResponseError::Decode => self.transport("decode"),
            super::response::ResponseError::TooLarge => self
                .error(
                    "embedding.tei.response_too_large",
                    "TEI success response exceeded its byte limit",
                )
                .with_retry_policy(axon_error::retry::RetryPolicy::fail_fast()),
        }
    }

    pub(super) fn deferred_retry_after(&self, status: StatusCode, delay: Duration) -> ApiError {
        let until = i64::try_from(delay.as_secs())
            .ok()
            .and_then(chrono::Duration::try_seconds)
            .and_then(|duration| Utc::now().checked_add_signed(duration));
        let Some(until) = until else {
            // Reject impossible dates explicitly instead of wrapping, parking
            // indefinitely, or retrying earlier than the server requested.
            return self
                .error(
                    "embedding.tei.retry_after_invalid",
                    "TEI Retry-After exceeds the supported timestamp range",
                )
                .with_retry_policy(axon_error::retry::RetryPolicy::fail_fast());
        };
        self.status_error(status).with_provider_cooling(
            ProviderCooling::new(
                until.max(Utc::now() + chrono::Duration::seconds(TEI_COOLDOWN_SECS)),
            )
            .with_provider(&self.provider_id)
            .with_reason("tei_retry_after"),
        )
    }

    pub(super) fn with_exhausted_cooling(&self, err: ApiError) -> ApiError {
        err.with_provider_cooling(
            ProviderCooling::new(Utc::now() + chrono::Duration::seconds(TEI_COOLDOWN_SECS))
                .with_provider(&self.provider_id)
                .with_reason("tei_retry_exhausted"),
        )
    }

    pub(super) fn error(&self, code: &str, message: &str) -> ApiError {
        ApiError::new(code, ErrorStage::Embedding, message.to_string())
            .with_context("endpoint", ENDPOINT_MARKER)
            .with_provider_id(&self.provider_id)
    }

    pub(super) fn transport(&self, category: &str) -> ApiError {
        // reqwest Display can carry credentials, so only include the category.
        self.error(
            "embedding.tei.transport",
            &format!("TEI transport error ({category})"),
        )
    }

    pub(super) fn status_error(&self, status: StatusCode) -> ApiError {
        self.error(
            "embedding.tei.status",
            &format!("TEI returned status {}", status.as_u16()),
        )
        .with_context("status", status.as_u16().to_string())
    }
}
