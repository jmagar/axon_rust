use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::redact_url;
use crate::axon_cli::crates::core::health::{
    browser_backend_selection, browser_diagnostics_pattern, do_not_port_guardrails,
    webdriver_url_from_env, BrowserBackendSelection,
};
use crate::axon_cli::crates::core::ui::{muted, primary, status_text, symbol_for_status};
use crate::axon_cli::crates::jobs::batch_jobs::batch_doctor;
use crate::axon_cli::crates::jobs::crawl_jobs::doctor as crawl_doctor;
use crate::axon_cli::crates::jobs::embed_jobs::embed_doctor;
use crate::axon_cli::crates::jobs::extract_jobs::extract_doctor;
use serde_json::Value;
use std::env;
use std::error::Error;
use std::time::Duration;

fn with_path(base: &str, path: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{trimmed}{path}")
    } else {
        format!("{trimmed}/{path}")
    }
}

async fn probe_http(url: &str, paths: &[&str]) -> (bool, Option<String>) {
    if url.trim().is_empty() {
        return (false, Some("not configured".to_string()));
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        Ok(c) => c,
        Err(err) => return (false, Some(err.to_string())),
    };

    let mut last_error = None;
    for path in paths {
        let endpoint = with_path(url, path);
        match client.get(endpoint).send().await {
            Ok(resp) => return (true, Some(format!("http {}", resp.status().as_u16()))),
            Err(err) => last_error = Some(err.to_string()),
        }
    }

    (false, last_error)
}

async fn probe_tei_info(url: &str) -> (Option<Value>, Option<String>) {
    if url.trim().is_empty() {
        return (None, Some("not configured".to_string()));
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        Ok(c) => c,
        Err(err) => return (None, Some(err.to_string())),
    };

    let mut last_error = None;
    for path in ["/info", "/v1/info"] {
        let endpoint = with_path(url, path);
        match client.get(endpoint).send().await {
            Ok(resp) if resp.status().is_success() => {
                let status = resp.status();
                match resp.json::<Value>().await {
                    Ok(json) => return (Some(json), Some(format!("{path} {status}"))),
                    Err(err) => last_error = Some(format!("{path} invalid json: {err}")),
                }
            }
            Ok(resp) => last_error = Some(format!("{path} {}", resp.status())),
            Err(err) => last_error = Some(err.to_string()),
        }
    }

    (None, last_error)
}

fn tei_model_from_info(info: &Value) -> Option<String> {
    if let Some(model_id) = info.get("model_id").and_then(Value::as_str) {
        return Some(model_id.to_string());
    }
    if let Some(model_name) = info.get("model_name").and_then(Value::as_str) {
        return Some(model_name.to_string());
    }
    if let Some(model) = info.get("model") {
        if let Some(name) = model.as_str() {
            return Some(name.to_string());
        }
        if let Some(name) = model.get("id").and_then(Value::as_str) {
            return Some(name.to_string());
        }
        if let Some(name) = model.get("name").and_then(Value::as_str) {
            return Some(name.to_string());
        }
    }
    None
}

fn tei_info_summary(info: &Value) -> Option<String> {
    let mut parts = Vec::new();
    for key in [
        "model_sha",
        "max_concurrent_requests",
        "max_client_batch_size",
        "max_batch_total_tokens",
        "max_input_tokens",
        "max_input_length",
    ] {
        if let Some(value) = info.get(key) {
            parts.push(format!("{key}={value}"));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn resolve_openai_model(cfg: &Config) -> String {
    if !cfg.openai_model.trim().is_empty() {
        return cfg.openai_model.clone();
    }

    env::var("OPENAI_MODEL").unwrap_or_default()
}

fn openai_state(cfg: &Config, openai_model: &str) -> (&'static str, bool) {
    let has_key = !cfg.openai_api_key.trim().is_empty();
    let has_model = !openai_model.trim().is_empty();
    let has_base = !cfg.openai_base_url.trim().is_empty();

    if has_key && has_model && has_base {
        ("configured", true)
    } else if has_key && has_model {
        ("configured (default base URL)", true)
    } else {
        ("not configured", false)
    }
}

pub async fn run_doctor(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let webdriver_url = webdriver_url_from_env();
    let diagnostics = browser_diagnostics_pattern();

    let (
        crawl_report,
        batch_report,
        extract_report,
        embed_report,
        tei_probe,
        tei_info_probe,
        qdrant_probe,
        webdriver_probe,
    ) = spider::tokio::join!(
        crawl_doctor(cfg),
        batch_doctor(cfg),
        extract_doctor(cfg),
        embed_doctor(cfg),
        probe_http(&cfg.tei_url, &["/health", "/"]),
        probe_tei_info(&cfg.tei_url),
        probe_http(&cfg.qdrant_url, &["/healthz", "/"]),
        async {
            match webdriver_url.as_deref() {
                Some(url) => Some(probe_http(url, &["/status", "/wd/hub/status"]).await),
                None => None,
            }
        },
    );

    let crawl_report = crawl_report?;
    let batch_report = batch_report?;
    let extract_report = extract_report?;
    let embed_report = embed_report?;

    let postgres_ok = crawl_report["postgres_ok"].as_bool().unwrap_or(false);
    let redis_ok = crawl_report["redis_ok"].as_bool().unwrap_or(false);
    let amqp_ok = crawl_report["amqp_ok"].as_bool().unwrap_or(false);
    let tei_ok = tei_probe.0;
    let tei_detail = tei_probe.1;
    let tei_info = tei_info_probe.0;
    let tei_info_detail = tei_info_probe.1;
    let tei_model = tei_info.as_ref().and_then(tei_model_from_info);
    let tei_summary = tei_info.as_ref().and_then(tei_info_summary);
    let qdrant_ok = qdrant_probe.0;
    let qdrant_detail = qdrant_probe.1;
    let webdriver_configured = webdriver_url.is_some();
    let webdriver_ok = webdriver_probe
        .as_ref()
        .map(|probe| probe.0)
        .unwrap_or(false);
    let webdriver_detail = webdriver_probe.and_then(|probe| probe.1);
    let backend_selection = browser_backend_selection(true, webdriver_configured, webdriver_ok);
    let backend_selection_label = match backend_selection {
        BrowserBackendSelection::Chrome => "chrome",
        BrowserBackendSelection::WebDriverFallback => "webdriver",
    };

    let openai_model = resolve_openai_model(cfg);
    let openai = openai_state(cfg, &openai_model);

    let pipelines = serde_json::json!({
        "crawl": crawl_report["all_ok"].as_bool().unwrap_or(false),
        "batch": batch_report["all_ok"].as_bool().unwrap_or(false),
        "extract": extract_report["all_ok"].as_bool().unwrap_or(false),
        "embed": embed_report["all_ok"].as_bool().unwrap_or(false),
    });

    let services = serde_json::json!({
        "postgres": { "ok": postgres_ok, "url": redact_url(&cfg.pg_url) },
        "redis": { "ok": redis_ok, "url": redact_url(&cfg.redis_url) },
        "amqp": { "ok": amqp_ok, "url": redact_url(&cfg.amqp_url) },
        "tei": {
            "ok": tei_ok,
            "url": cfg.tei_url,
            "detail": tei_detail,
            "info_detail": tei_info_detail,
            "model": tei_model,
            "summary": tei_summary,
            "info": tei_info
        },
        "qdrant": { "ok": qdrant_ok, "url": cfg.qdrant_url, "detail": qdrant_detail },
        "webdriver": {
            "ok": webdriver_ok,
            "configured": webdriver_configured,
            "url": webdriver_url,
            "detail": webdriver_detail
        },
        "openai": { "ok": openai.1, "state": openai.0, "base_url": cfg.openai_base_url, "model": openai_model },
    });

    let browser_runtime = serde_json::json!({
        "selection": backend_selection_label,
        "fallback_enabled": webdriver_configured,
        "fallback_ready": webdriver_ok,
        "diagnostics": {
            "enabled": diagnostics.enabled,
            "screenshot": diagnostics.screenshot,
            "events": diagnostics.events,
            "output_dir": diagnostics.output_dir,
        },
        "do_not_port_guardrails": do_not_port_guardrails(),
    });

    let all_ok = pipelines["crawl"].as_bool().unwrap_or(false)
        && pipelines["batch"].as_bool().unwrap_or(false)
        && pipelines["extract"].as_bool().unwrap_or(false)
        && pipelines["embed"].as_bool().unwrap_or(false)
        && tei_ok
        && qdrant_ok;

    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "services": services,
                "pipelines": pipelines,
                "browser_runtime": browser_runtime,
                "all_ok": all_ok
            }))?
        );
        return Ok(());
    }

    println!("{}", primary("Doctor Report"));
    println!();
    println!("{}", primary("Services"));
    println!(
        "  {} postgres {} {}",
        symbol_for_status(if postgres_ok { "completed" } else { "failed" }),
        status_text(if postgres_ok { "completed" } else { "failed" }),
        muted(&redact_url(&cfg.pg_url)),
    );
    println!(
        "  {} redis {} {}",
        symbol_for_status(if redis_ok { "completed" } else { "failed" }),
        status_text(if redis_ok { "completed" } else { "failed" }),
        muted(&redact_url(&cfg.redis_url)),
    );
    println!(
        "  {} amqp {} {}",
        symbol_for_status(if amqp_ok { "completed" } else { "failed" }),
        status_text(if amqp_ok { "completed" } else { "failed" }),
        muted(&redact_url(&cfg.amqp_url)),
    );
    println!(
        "  {} tei {} {}",
        symbol_for_status(if tei_ok { "completed" } else { "failed" }),
        status_text(if tei_ok { "completed" } else { "failed" }),
        muted(&tei_detail.unwrap_or_else(|| "unreachable".to_string())),
    );
    if let Some(model) = tei_model.as_deref() {
        println!("    model: {}", muted(model));
    }
    if let Some(summary) = tei_summary.as_deref() {
        println!("    info: {}", muted(summary));
    } else if let Some(info_detail) = tei_info_detail.as_deref() {
        println!("    info: {}", muted(info_detail));
    }
    println!(
        "  {} qdrant {} {}",
        symbol_for_status(if qdrant_ok { "completed" } else { "failed" }),
        status_text(if qdrant_ok { "completed" } else { "failed" }),
        muted(&qdrant_detail.unwrap_or_else(|| "unreachable".to_string())),
    );
    println!(
        "  {} webdriver {} {}",
        symbol_for_status(if webdriver_ok { "completed" } else { "failed" }),
        status_text(if webdriver_ok { "completed" } else { "failed" }),
        muted(&if webdriver_configured {
            format!(
                "{} ({})",
                redact_url(webdriver_url.as_deref().unwrap_or("")),
                webdriver_detail.unwrap_or_else(|| "unreachable".to_string())
            )
        } else {
            "not configured (optional fallback)".to_string()
        }),
    );
    println!(
        "  {} openai {} {}",
        symbol_for_status(if openai.1 { "completed" } else { "failed" }),
        status_text(if openai.1 { "completed" } else { "failed" }),
        muted(openai.0),
    );
    println!();
    println!("{}", primary("Pipelines"));
    for (name, ok) in [
        ("crawl", pipelines["crawl"].as_bool().unwrap_or(false)),
        ("batch", pipelines["batch"].as_bool().unwrap_or(false)),
        ("extract", pipelines["extract"].as_bool().unwrap_or(false)),
        ("embed", pipelines["embed"].as_bool().unwrap_or(false)),
    ] {
        println!(
            "  {} {} {}",
            symbol_for_status(if ok { "completed" } else { "failed" }),
            name,
            status_text(if ok { "completed" } else { "failed" }),
        );
    }
    println!();
    println!("{}", primary("Browser Runtime"));
    println!("  selection: {}", muted(backend_selection_label));
    println!(
        "  diagnostics: {} (screenshot={} events={} dir={})",
        muted(if diagnostics.enabled {
            "enabled"
        } else {
            "disabled"
        }),
        diagnostics.screenshot,
        diagnostics.events,
        diagnostics.output_dir
    );
    println!("  do-not-port guardrails:");
    for item in do_not_port_guardrails() {
        println!("    - {}", muted(item));
    }
    println!();
    println!(
        "{} overall {}",
        symbol_for_status(if all_ok { "completed" } else { "failed" }),
        status_text(if all_ok { "completed" } else { "failed" }),
    );

    Ok(())
}
