use crate::crates::core::config::Config;
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{muted, primary, print_phase};
use spider_agent::{Agent, ResearchOptions, SearchOptions, TimeRange};
use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

pub async fn research_payload(
    cfg: &Config,
    query: &str,
    limit: usize,
    offset: usize,
    time_range: Option<TimeRange>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let started = Instant::now();
    if cfg.tavily_api_key.is_empty() {
        return Err("research requires TAVILY_API_KEY — set it in .env".into());
    }
    if cfg.openai_base_url.is_empty() || cfg.openai_model.is_empty() {
        return Err("research requires OPENAI_BASE_URL and OPENAI_MODEL — set them in .env".into());
    }

    let base = cfg.openai_base_url.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        return Err(
            "OPENAI_BASE_URL should not include /chat/completions — set the base URL only (e.g. http://host/v1)".into()
        );
    }
    let _ = spider::url::Url::parse(base)
        .map_err(|e| format!("invalid OPENAI_BASE_URL '{base}': {e}"))?;
    let llm_url = format!("{base}/chat/completions");

    let agent = Agent::builder()
        .with_openai_compatible(llm_url, &cfg.openai_api_key, &cfg.openai_model)
        .with_search_tavily(&cfg.tavily_api_key)
        .build()?;

    let extraction_prompt =
        format!("Extract key facts, details, and insights relevant to: {query}");
    let mut search_options = SearchOptions::new().with_limit((limit + offset).clamp(1, 100));
    if let Some(tr) = time_range {
        search_options = search_options.with_time_range(tr);
    }

    let research = agent
        .research(
            query,
            ResearchOptions::new()
                .with_max_pages((limit + offset).clamp(1, 100))
                .with_search_options(search_options)
                .with_extraction_prompt(extraction_prompt)
                .with_synthesize(true),
        )
        .await?;

    let search_results = research
        .search_results
        .results
        .iter()
        .skip(offset)
        .take(limit)
        .map(|r| {
            serde_json::json!({
                "position": r.position,
                "title": r.title,
                "url": r.url,
                "snippet": r.snippet,
            })
        })
        .collect::<Vec<_>>();

    let extractions = research
        .extractions
        .iter()
        .skip(offset)
        .take(limit)
        .map(|e| {
            serde_json::json!({
                "url": e.url,
                "title": e.title,
                "extracted": e.extracted,
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "query": query,
        "limit": limit,
        "offset": offset,
        "search_results": search_results,
        "extractions": extractions,
        "summary": research.summary,
        "usage": {
            "prompt_tokens": research.usage.prompt_tokens,
            "completion_tokens": research.usage.completion_tokens,
            "total_tokens": research.usage.total_tokens,
        },
        "timing_ms": {
            "total": started.elapsed().as_millis(),
        },
    }))
}

pub async fn run_research(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.tavily_api_key.is_empty() {
        return Err("research requires TAVILY_API_KEY — set it in .env".into());
    }
    if cfg.openai_base_url.is_empty() || cfg.openai_model.is_empty() {
        return Err("research requires OPENAI_BASE_URL and OPENAI_MODEL — set them in .env".into());
    }

    let query = if let Some(q) = &cfg.query {
        q.clone()
    } else if !cfg.positional.is_empty() {
        cfg.positional.join(" ")
    } else {
        return Err("research requires a query (positional or --query)".into());
    };

    print_phase("◐", "Researching", &query);
    println!("  {} {}", muted("provider=tavily model="), cfg.openai_model);
    println!();

    let started = Instant::now();
    let running = Arc::new(AtomicBool::new(true));
    let running_tick = Arc::clone(&running);
    let tick_started = started;
    let ticker = if !cfg.json_output {
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if !running_tick.load(Ordering::Relaxed) {
                    break;
                }
                eprintln!(
                    "  {} research in progress... {}ms",
                    muted("progress"),
                    tick_started.elapsed().as_millis()
                );
            }
        }))
    } else {
        None
    };

    let payload = research_payload(cfg, &query, cfg.search_limit, 0, None).await;
    running.store(false, Ordering::Relaxed);
    if let Some(t) = ticker {
        let _ = t.await;
    }
    let payload = payload?;

    let search_results = payload["search_results"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let extractions = payload["extractions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let summary = payload["summary"].as_str();
    let prompt_tokens = payload["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let completion_tokens = payload["usage"]["completion_tokens"].as_u64().unwrap_or(0);
    let total_tokens = payload["usage"]["total_tokens"].as_u64().unwrap_or(0);
    let total_ms = started.elapsed().as_millis();

    println!("{} {}", primary("Search Results:"), search_results.len());
    println!();

    println!("{} {}", primary("Pages Extracted:"), extractions.len());
    println!();

    for (i, extraction) in extractions.iter().enumerate() {
        let title = extraction["title"].as_str().unwrap_or("");
        let url = extraction["url"].as_str().unwrap_or("");
        println!("{}. {}", i + 1, primary(title));
        println!("   {}", muted(url));
        let preview = serde_json::to_string(&extraction["extracted"])
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect::<String>();
        let preview = preview.trim();
        if preview.is_empty() || preview == "null" || preview == "{}" {
            println!("   {}", muted("(no data extracted)"));
        } else {
            println!("   {preview}");
        }
        println!();
    }

    if let Some(summary) = summary {
        println!("{}", primary("=== Summary ==="));
        println!("{summary}");
        println!();
    }

    if total_tokens > 0 {
        println!(
            "  {} prompt={} completion={} total={}",
            muted("tokens"),
            prompt_tokens,
            completion_tokens,
            total_tokens
        );
    }
    println!("  {} total={}ms", muted("timing"), total_ms);

    log_done("command=research complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crates::core::config::CommandKind;
    use crate::crates::jobs::common::test_config;

    fn make_research_cfg(tavily_key: &str, openai_url: &str, openai_model: &str) -> Config {
        let mut cfg = test_config("");
        cfg.command = CommandKind::Research;
        cfg.positional = vec!["test query".to_string()];
        cfg.tavily_api_key = tavily_key.to_string();
        cfg.openai_base_url = openai_url.to_string();
        cfg.openai_api_key = "test-key".to_string();
        cfg.openai_model = openai_model.to_string();
        cfg
    }

    #[tokio::test]
    async fn test_run_research_rejects_empty_tavily_key() {
        let cfg = make_research_cfg("", "http://localhost/v1", "gpt-4o-mini");
        let err = run_research(&cfg).await.unwrap_err();
        assert!(
            err.to_string().contains("TAVILY_API_KEY"),
            "expected TAVILY_API_KEY error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_research_rejects_empty_openai_config() {
        let cfg = make_research_cfg("tvly-key", "", "gpt-4o-mini");
        let err = run_research(&cfg).await.unwrap_err();
        assert!(
            err.to_string().contains("OPENAI_BASE_URL"),
            "expected OPENAI_BASE_URL error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_research_rejects_empty_openai_model() {
        let cfg = make_research_cfg("tvly-key", "http://localhost/v1", "");
        let err = run_research(&cfg).await.unwrap_err();
        assert!(
            err.to_string().contains("OPENAI_MODEL"),
            "expected OPENAI_MODEL error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_research_rejects_missing_query() {
        let mut cfg = make_research_cfg("tvly-key", "http://localhost/v1", "gpt-4o-mini");
        cfg.positional = vec![];
        cfg.query = None;
        let err = run_research(&cfg).await.unwrap_err();
        assert!(
            err.to_string().contains("query"),
            "expected query error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_research_rejects_double_chat_completions() {
        let cfg = make_research_cfg(
            "tvly-key",
            "http://localhost/v1/chat/completions",
            "gpt-4o-mini",
        );
        let err = run_research(&cfg).await.unwrap_err();
        assert!(
            err.to_string()
                .contains("should not include /chat/completions"),
            "expected /chat/completions validation error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_research_rejects_invalid_url() {
        let cfg = make_research_cfg("tvly-key", "not a valid url", "gpt-4o-mini");
        let err = run_research(&cfg).await.unwrap_err();
        assert!(
            err.to_string().contains("invalid OPENAI_BASE_URL"),
            "expected URL parse error, got: {err}"
        );
    }

    #[test]
    fn research_cfg_depth_defaults_to_none() {
        let cfg = make_research_cfg("tvly-key", "http://localhost/v1", "gpt-4o-mini");
        assert!(
            cfg.research_depth.is_none(),
            "research_depth should default to None"
        );
    }
}
