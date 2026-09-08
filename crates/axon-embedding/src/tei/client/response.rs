//! Keep response-body I/O inside the embedding attempt's retry boundary.

use serde::de::DeserializeOwned;

// Larger legitimate batches are split by the caller; the single-vector and
// model-info limits also bound dimension probing and provider-controlled JSON.
const EMBED_BODY_LIMIT: usize = 16 * 1024 * 1024;
pub(super) const INFO_BODY_LIMIT: usize = 1024 * 1024;

#[derive(Debug)]
pub(super) enum ResponseError {
    Transport(reqwest::Error),
    Decode,
    TooLarge,
}

impl From<reqwest::Error> for ResponseError {
    fn from(error: reqwest::Error) -> Self {
        Self::Transport(error)
    }
}

pub(super) enum EmbedResponse {
    Vectors(Vec<Vec<f32>>),
    Status(reqwest::Response),
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;

pub(super) async fn send_with_body(
    request: reqwest::RequestBuilder,
) -> Result<EmbedResponse, ResponseError> {
    let response = request.send().await?;
    if response.status().is_success() {
        Ok(EmbedResponse::Vectors(
            read_json(response, EMBED_BODY_LIMIT).await?,
        ))
    } else {
        Ok(EmbedResponse::Status(response))
    }
}

pub(super) async fn read_json<T: DeserializeOwned>(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<T, ResponseError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(ResponseError::TooLarge);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(ResponseError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| ResponseError::Decode)
}
