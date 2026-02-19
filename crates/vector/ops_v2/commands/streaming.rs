use crate::axon_cli::crates::core::config::Config;
use futures_util::StreamExt;
use std::error::Error;
use std::io::Write;

fn extract_sse_token(data: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(data).ok()?;
    value["choices"][0]["delta"]["content"]
        .as_str()
        .or_else(|| value["choices"][0]["message"]["content"].as_str())
        .or_else(|| value["choices"][0]["text"].as_str())
        .map(str::to_string)
}

fn process_sse_line(
    line: &str,
    answer: &mut String,
    print_tokens: bool,
    saw_stream_payload: &mut bool,
) -> Result<bool, Box<dyn Error>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with("data: ") {
        return Ok(false);
    }
    let data = trimmed.trim_start_matches("data: ").trim();
    if data.is_empty() {
        return Ok(false);
    }
    if data == "[DONE]" {
        return Ok(true);
    }

    if let Some(token) = extract_sse_token(data) {
        *saw_stream_payload = true;
        answer.push_str(&token);
        if print_tokens {
            print!("{token}");
            std::io::stdout().flush()?;
        }
    }
    Ok(false)
}

pub(super) async fn ask_llm_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
    print_tokens: bool,
) -> Result<String, Box<dyn Error>> {
    let mut req = client
        .post(format!(
            "{}/chat/completions",
            cfg.openai_base_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "model": cfg.openai_model,
            "messages": [
                {"role": "system", "content": "Answer only using provided context. Cite sources like [S1]."},
                {"role": "user", "content": format!("Question: {}\n\nContext:\n{}", query, context)}
            ],
            "temperature": 0.1,
            "stream": true
        }));

    if !cfg.openai_api_key.is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let response = req.send().await?;
    if !response.status().is_success() {
        return Err(format!("streaming request failed with status {}", response.status()).into());
    }

    let mut stream = response.bytes_stream();
    let mut answer = String::new();
    let mut pending = String::new();
    let mut saw_stream_payload = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        pending.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_idx) = pending.find('\n') {
            let mut line = pending[..newline_idx].to_string();
            pending.drain(..=newline_idx);
            if line.ends_with('\r') {
                let _ = line.pop();
            }
            let done = process_sse_line(&line, &mut answer, print_tokens, &mut saw_stream_payload)?;
            if done {
                return Ok(answer);
            }
        }
    }

    if !pending.trim().is_empty() {
        let _ = process_sse_line(&pending, &mut answer, print_tokens, &mut saw_stream_payload)?;
    }

    if saw_stream_payload {
        return Ok(answer);
    }

    Err("streaming response returned no token payload".into())
}

pub(super) async fn ask_llm_non_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
) -> Result<String, Box<dyn Error>> {
    let mut req = client
        .post(format!(
            "{}/chat/completions",
            cfg.openai_base_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "model": cfg.openai_model,
            "messages": [
                {"role": "system", "content": "Answer only using provided context. Cite sources like [S1]."},
                {"role": "user", "content": format!("Question: {}\n\nContext:\n{}", query, context)}
            ],
            "temperature": 0.1
        }));

    if !cfg.openai_api_key.is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    Ok(json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no answer)")
        .to_string())
}
