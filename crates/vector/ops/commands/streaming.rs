use crate::crates::core::config::Config;
use futures_util::StreamExt;
use std::error::Error;
use std::io::Write;

/// Build a POST request to the OpenAI-compatible chat completions endpoint with
/// optional bearer auth. Callers chain `.json(...)` to attach the request body.
pub(super) fn build_openai_chat_request(
    client: &reqwest::Client,
    cfg: &Config,
) -> reqwest::RequestBuilder {
    let mut req = client.post(format!(
        "{}/chat/completions",
        cfg.openai_base_url.trim_end_matches('/')
    ));
    if !cfg.openai_api_key.trim().is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }
    req
}

/// Context for LLM judge comparison between RAG and baseline answers.
pub(crate) struct JudgeContext<'a> {
    pub query: &'a str,
    pub rag_answer: &'a str,
    pub baseline_answer: &'a str,
    pub reference_chunks: &'a str,
    pub rag_sources_list: &'a str,
    pub ref_quality_note: &'a str,
    pub rag_elapsed_ms: u128,
    pub baseline_elapsed_ms: u128,
    pub source_count: usize,
    pub context_chars: usize,
}

fn judge_system_prompt() -> &'static str {
    "You are an expert evaluator with access to authoritative reference material.\n\
Compare two AI responses to the same question.\n\
\n\
IMPORTANT INSTRUCTIONS:\n\
- Do NOT score higher simply because an answer is longer or more technical. Concise and accurate beats verbose and wandering.\n\
- First, enumerate the key factual claims in each answer. Then verify each claim against the Reference Material using [R#] citations.\n\
- If reference chunks contain version numbers or dates, note whether the baseline answer may be out of date relative to the indexed material.\n\
\n\
Produce your analysis in this EXACT format:\n\
\n\
## Accuracy        RAG: X/5 | Baseline: X/5\n\
[Reasoning with [R#] citations for specific claims. Note any factual errors or omissions.]\n\
\n\
## Relevance       RAG: X/5 | Baseline: X/5\n\
[Did each answer address what was actually asked?]\n\
\n\
## Completeness    RAG: X/5 | Baseline: X/5\n\
[Did each answer cover the important details?]\n\
\n\
## Specificity     RAG: X/5 | Baseline: X/5\n\
[Did each answer give concrete, actionable information?]\n\
\n\
## Timing\n\
[Was the RAG latency overhead justified by the quality improvement?]\n\
\n\
## Did RAG Add Value?\n\
YES/NO — [Did the indexed knowledge base provide information the LLM could not have had from training alone? Be specific.]\n\
\n\
## Verdict\n\
[1-2 sentences: which response is better overall and why?]"
}

fn judge_user_msg(ctx: &JudgeContext<'_>) -> String {
    format!(
        "Question: {query}\n\n\
## RAG Answer (WITH context — {source_count} sources, {context_chars} chars, {rag_ms}ms)\n\
Sources the RAG answer was built from:\n{rag_sources_list}\n\n\
{rag_answer}\n\n\
## Baseline Answer (WITHOUT context, {baseline_ms}ms)\n\
{baseline_answer}\n\n\
## Reference Material (independent retrieval for accuracy grounding)\n\
{ref_quality_note}\
{reference_chunks}\n\n\
Analyze and compare the two responses following the format in your instructions.",
        query = ctx.query,
        source_count = ctx.source_count,
        context_chars = ctx.context_chars,
        rag_ms = ctx.rag_elapsed_ms,
        rag_sources_list = ctx.rag_sources_list,
        rag_answer = ctx.rag_answer,
        baseline_ms = ctx.baseline_elapsed_ms,
        baseline_answer = ctx.baseline_answer,
        ref_quality_note = ctx.ref_quality_note,
        reference_chunks = ctx.reference_chunks,
    )
}

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

async fn run_sse_stream(
    req: reqwest::RequestBuilder,
    print_tokens: bool,
) -> Result<String, Box<dyn Error>> {
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

    if saw_stream_payload && !answer.trim().is_empty() {
        return Ok(answer);
    }

    Err("streaming response returned no token payload".into())
}

pub(crate) async fn ask_llm_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
    print_tokens: bool,
) -> Result<String, Box<dyn Error>> {
    let req = build_openai_chat_request(client, cfg).json(&serde_json::json!({
        "model": cfg.openai_model,
        "messages": [
            {"role": "system", "content": "You are a precise technical research assistant. Answer questions using the retrieved source documents when they are relevant.\n\nSTEP 1 — RELEVANCE CHECK: Before answering, assess whether the retrieved documents genuinely address the question. Look for topical overlap beyond keyword coincidence (e.g., a doc about implementing a library's internal HTTP adapter is NOT relevant to a question about how to use HTTP clients in general).\n\nSTEP 2 — ANSWER based on your assessment:\n\nIF SOURCES ARE RELEVANT:\n1. CITATIONS — Cite inline immediately after each claim using [S#] labels. When multiple sources support the same point, cite all: [S1][S3]. Never make a claim without a citation.\n2. FOOTER — After your answer, add a \"## Sources\" section listing each cited source number and its URL.\n3. SYNTHESIS — Integrate information from multiple sources into a unified answer.\n4. GAPS — State explicitly what the sources cover and what they do not.\n5. PRECISION — Include exact values, function names, file paths, and configuration keys when the sources provide them.\n\nIF SOURCES ARE NOT RELEVANT (documents discuss a related but different topic than what was asked):\n- Open with: \"The indexed knowledge base does not contain directly relevant information for this question.\"\n- Then provide a complete, accurate answer in a section labeled \"## Answer (from training knowledge)\".\n- Do NOT cite [S#] for claims not supported by the retrieved sources."},
            {"role": "user", "content": format!("Question: {}\n\nContext:\n{}", query, context)}
        ],
        "temperature": 0.1,
        "stream": true
    }));

    run_sse_stream(req, print_tokens).await
}

pub(crate) async fn ask_llm_non_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
) -> Result<String, Box<dyn Error>> {
    let req = build_openai_chat_request(client, cfg).json(&serde_json::json!({
        "model": cfg.openai_model,
        "messages": [
            {"role": "system", "content": "You are a precise technical research assistant. Answer questions using the retrieved source documents when they are relevant.\n\nSTEP 1 — RELEVANCE CHECK: Before answering, assess whether the retrieved documents genuinely address the question. Look for topical overlap beyond keyword coincidence (e.g., a doc about implementing a library's internal HTTP adapter is NOT relevant to a question about how to use HTTP clients in general).\n\nSTEP 2 — ANSWER based on your assessment:\n\nIF SOURCES ARE RELEVANT:\n1. CITATIONS — Cite inline immediately after each claim using [S#] labels. When multiple sources support the same point, cite all: [S1][S3]. Never make a claim without a citation.\n2. FOOTER — After your answer, add a \"## Sources\" section listing each cited source number and its URL.\n3. SYNTHESIS — Integrate information from multiple sources into a unified answer.\n4. GAPS — State explicitly what the sources cover and what they do not.\n5. PRECISION — Include exact values, function names, file paths, and configuration keys when the sources provide them.\n\nIF SOURCES ARE NOT RELEVANT (documents discuss a related but different topic than what was asked):\n- Open with: \"The indexed knowledge base does not contain directly relevant information for this question.\"\n- Then provide a complete, accurate answer in a section labeled \"## Answer (from training knowledge)\".\n- Do NOT cite [S#] for claims not supported by the retrieved sources."},
            {"role": "user", "content": format!("Question: {}\n\nContext:\n{}", query, context)}
        ],
        "temperature": 0.1
    }));

    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    Ok(json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no answer)")
        .to_string())
}

pub(crate) async fn baseline_llm_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    print_tokens: bool,
) -> Result<String, Box<dyn Error>> {
    let req = build_openai_chat_request(client, cfg).json(&serde_json::json!({
        "model": cfg.openai_model,
        "messages": [
            {"role": "system", "content": "You are a knowledgeable technical assistant. Answer the following question accurately and thoroughly, drawing on your full training knowledge. Where you are uncertain or your knowledge may be outdated, say so explicitly rather than presenting uncertain information as fact. For technical questions, be specific: include exact values, function names, and configuration details where you know them."},
            {"role": "user", "content": query}
        ],
        "temperature": 0.1,
        "stream": true
    }));

    run_sse_stream(req, print_tokens).await
}

pub(crate) async fn baseline_llm_non_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
) -> Result<String, Box<dyn Error>> {
    let req = build_openai_chat_request(client, cfg).json(&serde_json::json!({
        "model": cfg.openai_model,
        "messages": [
            {"role": "system", "content": "You are a knowledgeable technical assistant. Answer the following question accurately and thoroughly, drawing on your full training knowledge. Where you are uncertain or your knowledge may be outdated, say so explicitly rather than presenting uncertain information as fact. For technical questions, be specific: include exact values, function names, and configuration details where you know them."},
            {"role": "user", "content": query}
        ],
        "temperature": 0.1
    }));

    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    Ok(json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no answer)")
        .to_string())
}

pub(crate) async fn judge_llm_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    ctx: &JudgeContext<'_>,
    print_tokens: bool,
) -> Result<String, Box<dyn Error>> {
    let user_msg = judge_user_msg(ctx);
    let req = build_openai_chat_request(client, cfg).json(&serde_json::json!({
        "model": cfg.openai_model,
        "messages": [
            {"role": "system", "content": judge_system_prompt()},
            {"role": "user", "content": user_msg}
        ],
        "temperature": 0.3,
        "stream": true
    }));
    run_sse_stream(req, print_tokens).await
}

pub(crate) async fn judge_llm_non_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    ctx: &JudgeContext<'_>,
) -> Result<String, Box<dyn Error>> {
    let user_msg = judge_user_msg(ctx);
    let req = build_openai_chat_request(client, cfg).json(&serde_json::json!({
        "model": cfg.openai_model,
        "messages": [
            {"role": "system", "content": judge_system_prompt()},
            {"role": "user", "content": user_msg}
        ],
        "temperature": 0.3
    }));
    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    Ok(json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no analysis)")
        .to_string())
}
