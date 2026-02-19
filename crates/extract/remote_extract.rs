use crate::axon_cli::crates::core::http::http_client;
use spider::tokio;
use spider::website::Website;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct ExtractRun {
    pub start_url: String,
    pub pages_visited: usize,
    pub pages_with_data: usize,
    pub results: Vec<serde_json::Value>,
}

fn collect_items(value: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(arr) = value.get("results").and_then(|r| r.as_array()) {
        arr.clone()
    } else if let Some(arr) = value.as_array() {
        arr.clone()
    } else if !value.is_null() && value != &serde_json::Value::Object(serde_json::Map::new()) {
        vec![value.clone()]
    } else {
        vec![]
    }
}

async fn extract_items_fallback(
    client: &reqwest::Client,
    api_url: &str,
    openai_api_key: &str,
    openai_model: &str,
    prompt: &str,
    page_url: &str,
    html: &str,
) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {
    let trimmed_html: String = html.chars().take(20_000).collect();
    let response = client
        .post(api_url)
        .bearer_auth(openai_api_key)
        .json(&serde_json::json!({
            "model": openai_model,
            "messages": [
                {
                    "role": "system",
                    "content": format!(
                        "{} Return JSON with a top-level key \"results\" containing an array of extracted items.",
                        prompt
                    )
                },
                {
                    "role": "user",
                    "content": format!("URL: {}\n\nHTML:\n{}", page_url, trimmed_html)
                }
            ],
            "response_format": {"type": "json_object"},
            "temperature": 0.1
        }))
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = response.json().await?;
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");
    let parsed = serde_json::from_str::<serde_json::Value>(content).unwrap_or_default();
    Ok(collect_items(&parsed))
}

pub async fn run_remote_extract(
    start_url: &str,
    prompt: &str,
    limit: u32,
    openai_base_url: &str,
    openai_api_key: &str,
    openai_model: &str,
) -> Result<ExtractRun, Box<dyn Error>> {
    if openai_base_url.is_empty() {
        return Err("OPENAI_BASE_URL is required for extract".into());
    }
    if openai_api_key.is_empty() {
        return Err("OPENAI_API_KEY is required for extract".into());
    }
    if openai_model.is_empty() {
        return Err("OPENAI_MODEL is required for extract".into());
    }

    let api_url = format!("{}/chat/completions", openai_base_url.trim_end_matches('/'));
    let api_key = openai_api_key.to_string();
    let model = openai_model.to_string();
    let prompt_text = prompt.to_string();

    let mut website = Website::new(start_url)
        .with_limit(limit)
        .build()
        .map_err(|_| "build website")?;

    let mut rx = website.subscribe(16).ok_or("subscribe failed")?;
    let api_url_clone = api_url.clone();
    let client = http_client()?.clone();

    let collect = tokio::spawn(async move {
        let mut all_results: Vec<serde_json::Value> = vec![];
        let mut pages_with_data = 0usize;

        while let Ok(page) = rx.recv().await {
            let page_url = page.get_url().to_string();
            let html = page.get_html();
            if html.is_empty() {
                continue;
            }

            if let Ok(items) = extract_items_fallback(
                &client,
                &api_url_clone,
                &api_key,
                &model,
                &prompt_text,
                &page_url,
                &html,
            )
            .await
            {
                if !items.is_empty() {
                    pages_with_data += 1;
                    all_results.extend(items);
                }
            }
        }

        (all_results, pages_with_data)
    });

    website.crawl_raw().await;
    website.unsubscribe();

    let (results, pages_with_data) = collect.await?;
    let pages_visited: usize = website.get_all_links_visited().await.len();

    Ok(ExtractRun {
        start_url: start_url.to_string(),
        pages_visited,
        pages_with_data,
        results,
    })
}
