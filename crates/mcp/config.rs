use crate::crates::core::config::Config;
use crate::crates::core::config::parse::normalize_local_service_url;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

pub fn load_mcp_config() -> Config {
    let mut cfg = Config::default();

    if let Some(v) = env("AXON_PG_URL") {
        cfg.pg_url = normalize_local_service_url(v);
    }
    if let Some(v) = env("AXON_REDIS_URL") {
        cfg.redis_url = normalize_local_service_url(v);
    }
    if let Some(v) = env("AXON_AMQP_URL") {
        cfg.amqp_url = normalize_local_service_url(v);
    }
    if let Some(v) = env("QDRANT_URL") {
        cfg.qdrant_url = normalize_local_service_url(v);
    }
    if let Some(v) = env("TEI_URL") {
        cfg.tei_url = v;
    }
    if let Some(v) = env("OPENAI_BASE_URL") {
        cfg.openai_base_url = v;
    }
    if let Some(v) = env("OPENAI_API_KEY") {
        cfg.openai_api_key = v;
    }
    if let Some(v) = env("OPENAI_MODEL") {
        cfg.openai_model = v;
    }
    if let Some(v) = env("TAVILY_API_KEY") {
        cfg.tavily_api_key = v;
    }

    if let Some(v) = env("AXON_COLLECTION") {
        cfg.collection = v;
    }
    if let Some(v) = env("AXON_CRAWL_QUEUE") {
        cfg.crawl_queue = v;
    }
    if let Some(v) = env("AXON_EXTRACT_QUEUE") {
        cfg.extract_queue = v;
    }
    if let Some(v) = env("AXON_EMBED_QUEUE") {
        cfg.embed_queue = v;
    }
    if let Some(v) = env("AXON_INGEST_QUEUE") {
        cfg.ingest_queue = v;
    }
    if let Some(v) = env("AXON_REFRESH_QUEUE") {
        cfg.refresh_queue = v;
    }

    // Ask authoritative tuning
    if let Some(v) = env("AXON_ASK_AUTHORITATIVE_DOMAINS") {
        cfg.ask_authoritative_domains = v
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Some(v) = env("AXON_ASK_AUTHORITATIVE_BOOST") {
        if let Ok(f) = v.parse::<f64>() {
            cfg.ask_authoritative_boost = f.clamp(0.0, 0.5);
        }
    }
    if let Some(v) = env("AXON_ASK_AUTHORITATIVE_ALLOWLIST") {
        cfg.ask_authoritative_allowlist = v
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Some(v) = env("AXON_ASK_MIN_CITATIONS_NONTRIVIAL") {
        if let Ok(n) = v.parse::<usize>() {
            cfg.ask_min_citations_nontrivial = n.clamp(1, 5);
        }
    }

    if let Some(v) = env("AXON_CHROME_REMOTE_URL") {
        cfg.chrome_remote_url = Some(normalize_local_service_url(v));
    }

    if let Some(v) = env("GITHUB_TOKEN") {
        cfg.github_token = Some(v);
    }
    if let Some(v) = env("REDDIT_CLIENT_ID") {
        cfg.reddit_client_id = Some(v);
    }
    if let Some(v) = env("REDDIT_CLIENT_SECRET") {
        cfg.reddit_client_secret = Some(v);
    }

    cfg.json_output = true;
    cfg.wait = false;
    cfg
}
