use crate::crates::core::config::Config;
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{muted, primary, print_phase};
use spider_agent::{Agent, ResearchOptions, SearchOptions};
use std::error::Error;

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

    // Validate OPENAI_BASE_URL before constructing the LLM endpoint.
    let base = cfg.openai_base_url.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        return Err(
            "OPENAI_BASE_URL should not include /chat/completions — set the base URL only (e.g. http://host/v1)".into()
        );
    }
    let _ = spider::url::Url::parse(base)
        .map_err(|e| format!("invalid OPENAI_BASE_URL '{base}': {e}"))?;

    // spider_agent's with_openai_compatible expects the full endpoint URL.
    let llm_url = format!("{base}/chat/completions");

    let agent = Agent::builder()
        .with_openai_compatible(llm_url, &cfg.openai_api_key, &cfg.openai_model)
        .with_search_tavily(&cfg.tavily_api_key)
        .build()?;

    let extraction_prompt =
        format!("Extract key facts, details, and insights relevant to: {query}");

    // TODO: cfg.research_depth — ResearchOptions::with_depth not available in current spider_agent version
    let research = agent
        .research(
            &query,
            ResearchOptions::new()
                .with_max_pages(cfg.search_limit)
                .with_search_options(SearchOptions::new().with_limit(cfg.search_limit))
                .with_extraction_prompt(extraction_prompt)
                .with_synthesize(true),
        )
        .await?;

    println!(
        "{} {}",
        primary("Search Results:"),
        research.search_results.results.len()
    );
    println!();

    println!(
        "{} {}",
        primary("Pages Extracted:"),
        research.extractions.len()
    );
    println!();

    for (i, extraction) in research.extractions.iter().enumerate() {
        println!("{}. {}", i + 1, primary(&extraction.title));
        println!("   {}", muted(&extraction.url));
        let preview = serde_json::to_string(&extraction.extracted)
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

    if let Some(summary) = &research.summary {
        println!("{}", primary("=== Summary ==="));
        println!("{summary}");
        println!();
    }

    if research.usage.total_tokens > 0 {
        println!(
            "  {} prompt={} completion={} total={}",
            muted("tokens"),
            research.usage.prompt_tokens,
            research.usage.completion_tokens,
            research.usage.total_tokens
        );
    }

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
