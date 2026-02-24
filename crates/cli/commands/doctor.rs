mod render;

use crate::crates::cli::commands::probe::{probe_http, with_path};
use crate::crates::core::config::Config;
use crate::crates::core::content::redact_url;
use crate::crates::core::health::{
    browser_backend_selection, browser_diagnostics_pattern, webdriver_url_from_env,
    BrowserBackendSelection,
};
use crate::crates::core::http::build_client;
use crate::crates::jobs::common::count_stale_and_pending_jobs;
use crate::crates::jobs::crawl::doctor as crawl_doctor;
use crate::crates::jobs::embed::embed_doctor;
use crate::crates::jobs::extract::extract_doctor;
use crate::crates::jobs::ingest::ingest_doctor;
use render::render_doctor_report_human;
use serde_json::Value;
use std::env;
use std::error::Error;

async fn probe_tei_info(url: &str) -> (Option<Value>, Option<String>) {
    if url.trim().is_empty() {
        return (None, Some("not configured".to_string()));
    }

    // Short 4s timeout for health probes — intentionally not the global 30s client.
    let client = match build_client(4) {
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

/// Live probe: GET /models with a 3s timeout. Returns (ok, detail).
async fn probe_openai(cfg: &Config, openai_model: &str) -> (bool, String) {
    let base = cfg.openai_base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return (false, "not configured".to_string());
    }
    if openai_model.trim().is_empty() {
        return (false, "OPENAI_MODEL not set".to_string());
    }

    let client = match build_client(3) {
        Ok(c) => c,
        Err(e) => return (false, e.to_string()),
    };

    let url = format!("{base}/models");
    let mut req = client.get(&url);
    if !cfg.openai_api_key.trim().is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            (true, format!("http {} /models", resp.status().as_u16()))
        }
        Ok(resp) => (false, format!("http {} /models", resp.status().as_u16())),
        Err(e) => (false, e.to_string()),
    }
}

/// Probe the Chrome CDP management endpoint. Returns (ok, detail).
async fn probe_chrome(chrome_url: Option<&str>) -> (bool, Option<String>) {
    let url = match chrome_url {
        Some(u) if !u.trim().is_empty() => u,
        _ => return (false, None),
    };

    // CDP /json/version is the canonical health endpoint.
    let result = probe_http(url, &["/json/version", "/json"]).await;
    (result.0, result.1)
}

struct DoctorProbes {
    crawl_report: Value,
    extract_report: Value,
    embed_report: Value,
    ingest_report: Value,
    tei_probe: (bool, Option<String>),
    tei_info_probe: (Option<Value>, Option<String>),
    qdrant_probe: (bool, Option<String>),
    webdriver_probe: Option<(bool, Option<String>)>,
    chrome_probe: (bool, Option<String>),
    openai_probe: (bool, String),
    stale_jobs: Option<(i64, i64)>,
}

async fn gather_doctor_probes(
    cfg: &Config,
    webdriver_url: Option<&str>,
    openai_model: &str,
) -> Result<DoctorProbes, Box<dyn Error>> {
    let (
        crawl_report,
        extract_report,
        embed_report,
        ingest_report,
        tei_probe,
        tei_info_probe,
        qdrant_probe,
        webdriver_probe,
        chrome_probe,
        openai_probe,
        stale_jobs,
    ) = spider::tokio::join!(
        crawl_doctor(cfg),
        extract_doctor(cfg),
        embed_doctor(cfg),
        ingest_doctor(cfg),
        probe_http(&cfg.tei_url, &["/health", "/"]),
        probe_tei_info(&cfg.tei_url),
        probe_http(&cfg.qdrant_url, &["/healthz", "/"]),
        async {
            match webdriver_url {
                Some(url) => Some(probe_http(url, &["/status", "/wd/hub/status"]).await),
                None => None,
            }
        },
        probe_chrome(cfg.chrome_remote_url.as_deref()),
        probe_openai(cfg, openai_model),
        count_stale_and_pending_jobs(cfg, 15),
    );

    Ok(DoctorProbes {
        crawl_report: crawl_report?,
        extract_report: extract_report?,
        embed_report: embed_report?,
        ingest_report: ingest_report?,
        tei_probe,
        tei_info_probe,
        qdrant_probe,
        webdriver_probe,
        chrome_probe,
        openai_probe,
        stale_jobs,
    })
}

fn build_pipeline_status(probes: &DoctorProbes, openai_ok: bool) -> Value {
    // extract is degraded (not failed) when OpenAI is unconfigured — jobs will be
    // queued but will fail at the LLM step. Surface that here so the user sees it.
    let extract_infra_ok = probes.extract_report["all_ok"].as_bool().unwrap_or(false);
    serde_json::json!({
        "crawl": probes.crawl_report["all_ok"].as_bool().unwrap_or(false),
        "extract": extract_infra_ok,
        "extract_llm_ready": extract_infra_ok && openai_ok,
        "embed": probes.embed_report["all_ok"].as_bool().unwrap_or(false),
        "ingest": probes.ingest_report["all_ok"].as_bool().unwrap_or(false),
    })
}

fn browser_backend_label(selection: BrowserBackendSelection) -> &'static str {
    match selection {
        BrowserBackendSelection::Chrome => "chrome",
        BrowserBackendSelection::WebDriverFallback => "webdriver",
    }
}

fn build_browser_runtime(
    diagnostics: &crate::crates::core::health::BrowserDiagnosticsPattern,
    selection: BrowserBackendSelection,
    webdriver_configured: bool,
    webdriver_ok: bool,
) -> Value {
    serde_json::json!({
        "selection": browser_backend_label(selection),
        "fallback_enabled": webdriver_configured,
        "fallback_ready": webdriver_ok,
        "diagnostics": {
            "enabled": diagnostics.enabled,
            "screenshot": diagnostics.screenshot,
            "events": diagnostics.events,
            "output_dir": diagnostics.output_dir,
        },
    })
}

fn build_services_status(
    cfg: &Config,
    probes: &DoctorProbes,
    webdriver_url: Option<&str>,
    openai_model: &str,
) -> Value {
    let tei_info = probes.tei_info_probe.0.clone();
    let tei_info_detail = probes.tei_info_probe.1.clone();
    let tei_model = tei_info.as_ref().and_then(tei_model_from_info);
    let tei_summary = tei_info.as_ref().and_then(tei_info_summary);
    let (openai_live_ok, ref openai_live_detail) = probes.openai_probe;
    let (chrome_ok, ref chrome_detail) = probes.chrome_probe;

    serde_json::json!({
        "postgres": {
            "ok": probes.crawl_report["postgres_ok"].as_bool().unwrap_or(false),
            "url": redact_url(&cfg.pg_url),
        },
        "redis": {
            "ok": probes.crawl_report["redis_ok"].as_bool().unwrap_or(false),
            "url": redact_url(&cfg.redis_url),
        },
        "amqp": {
            "ok": probes.crawl_report["amqp_ok"].as_bool().unwrap_or(false),
            "url": redact_url(&cfg.amqp_url),
        },
        "tei": {
            "ok": probes.tei_probe.0,
            "url": cfg.tei_url,
            "detail": probes.tei_probe.1.clone(),
            "info_detail": tei_info_detail,
            "model": tei_model,
            "summary": tei_summary,
            "info": tei_info,
        },
        "qdrant": {
            "ok": probes.qdrant_probe.0,
            "url": cfg.qdrant_url,
            "detail": probes.qdrant_probe.1.clone(),
        },
        "webdriver": {
            "ok": probes.webdriver_probe.as_ref().map(|probe| probe.0).unwrap_or(false),
            "configured": webdriver_url.is_some(),
            "url": webdriver_url,
            "detail": probes.webdriver_probe.as_ref().and_then(|probe| probe.1.clone()),
        },
        "chrome": {
            "ok": chrome_ok,
            "configured": cfg.chrome_remote_url.is_some(),
            "url": cfg.chrome_remote_url,
            "detail": chrome_detail,
        },
        "openai": {
            "ok": openai_live_ok,
            "configured": !cfg.openai_base_url.trim().is_empty() && !openai_model.trim().is_empty(),
            "detail": openai_live_detail,
            "base_url": cfg.openai_base_url,
            "model": openai_model,
        },
    })
}

fn build_queue_names(cfg: &Config) -> Value {
    serde_json::json!({
        "crawl": cfg.crawl_queue,
        "extract": cfg.extract_queue,
        "embed": cfg.embed_queue,
        "ingest": cfg.ingest_queue,
    })
}

fn report_overall_ok(pipelines: &Value, tei_ok: bool, qdrant_ok: bool) -> bool {
    pipelines["crawl"].as_bool().unwrap_or(false)
        && pipelines["extract"].as_bool().unwrap_or(false)
        && pipelines["embed"].as_bool().unwrap_or(false)
        && pipelines["ingest"].as_bool().unwrap_or(false)
        && tei_ok
        && qdrant_ok
}

// NOTE: run_doctor delegates to build_doctor_report and only renders output.
// Keep probe logic centralized in build_doctor_report to avoid drift.
pub async fn build_doctor_report(cfg: &Config) -> Result<Value, Box<dyn Error>> {
    let webdriver_url = webdriver_url_from_env();
    let diagnostics = browser_diagnostics_pattern();
    let openai_model = resolve_openai_model(cfg);
    let probes = gather_doctor_probes(cfg, webdriver_url.as_deref(), &openai_model).await?;
    let (openai_live_ok, _) = probes.openai_probe.clone();
    let pipelines = build_pipeline_status(&probes, openai_live_ok);
    let services = build_services_status(cfg, &probes, webdriver_url.as_deref(), &openai_model);
    let queue_names = build_queue_names(cfg);

    let webdriver_configured = webdriver_url.is_some();
    let webdriver_ok = probes
        .webdriver_probe
        .as_ref()
        .map(|probe| probe.0)
        .unwrap_or(false);
    let backend_selection = browser_backend_selection(true, webdriver_configured, webdriver_ok);
    let browser_runtime = build_browser_runtime(
        &diagnostics,
        backend_selection,
        webdriver_configured,
        webdriver_ok,
    );
    let all_ok = report_overall_ok(&pipelines, probes.tei_probe.0, probes.qdrant_probe.0);

    let (stale_count, pending_count) = probes.stale_jobs.unwrap_or((0, 0));

    Ok(serde_json::json!({
        "services": services,
        "pipelines": pipelines,
        "queue_names": queue_names,
        "browser_runtime": browser_runtime,
        "stale_jobs": stale_count,
        "pending_jobs": pending_count,
        "all_ok": all_ok
    }))
}

pub async fn run_doctor(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let report = build_doctor_report(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render_doctor_report_human(&report);
    }
    Ok(())
}
