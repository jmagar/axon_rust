use super::OpenAiProviderError;
use futures_util::StreamExt;
use std::error::Error;

const BODY_LIMIT: usize = 16 * 1024 * 1024;
const FRAME_LIMIT: usize = 1024 * 1024;

fn exceeded() -> Box<dyn Error + Send + Sync> {
    Box::new(OpenAiProviderError {
        code: "provider.output_limit",
        message: "OpenAI-compatible response exceeded its byte limit".into(),
        retryable: false,
    })
}

pub(super) async fn read_body(
    response: reqwest::Response,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.len() > BODY_LIMIT.saturating_sub(body.len()) {
            return Err(exceeded());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Default)]
pub(super) struct StreamBudget {
    total: usize,
    line: usize,
}

impl StreamBudget {
    pub(super) fn accept(&mut self, bytes: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        if bytes.len() > BODY_LIMIT.saturating_sub(self.total) {
            return Err(exceeded());
        }
        self.total += bytes.len();
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            self.line += part.len();
            if self.line > FRAME_LIMIT {
                return Err(exceeded());
            }
            if part.last() == Some(&b'\n') {
                self.line = 0;
            }
        }
        Ok(())
    }
}
