use crate::crates::cli::commands::probe::{probe_http, with_path};
use crate::crates::core::config::Config;
use crate::crates::core::content::redact_url;
use crate::crates::core::health::{
    browser_backend_selection, browser_diagnostics_pattern, webdriver_url_from_env,
    BrowserBackendSelection,
};
use crate::crates::core::http::build_client;
use crate::crates::core::ui::{muted, primary, status_text, symbol_for_status};
use crate::crates::jobs::batch_jobs::batch_doctor;
use crate::crates::jobs::crawl_jobs::doctor as crawl_doctor;
use crate::crates::jobs::embed_jobs::embed_doctor;
use crate::crates::jobs::extract_jobs::extract_doctor;
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

struct DoctorProbes {
    crawl_report: Value,
    batch_report: Value,
    extract_report: Value,
    embed_report: Value,
    tei_probe: (bool, Option<String>),
    tei_info_probe: (Option<Value>, Option<String>),
    qdrant_probe: (bool, Option<String>),
    webdriver_probe: Option<(bool, Option<String>)>,
}

async fn gather_doctor_probes(
    cfg: &Config,
    webdriver_url: Option<&str>,
) -> Result<DoctorProbes, Box<dyn Error>> {
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
            match webdriver_url {
                Some(url) => Some(probe_http(url, &["/status", "/wd/hub/status"]).await),
                None => None,
            }
        },
    );

    Ok(DoctorProbes {
        crawl_report: crawl_report?,
        batch_report: batch_report?,
        extract_report: extract_report?,
        embed_report: embed_report?,
        tei_probe,
        tei_info_probe,
        qdrant_probe,
        webdriver_probe,
    })
}

fn build_pipeline_status(probes: &DoctorProbes) -> Value {
    serde_json::json!({
        "crawl": probes.crawl_report["all_ok"].as_bool().unwrap_or(false),
        "batch": probes.batch_report["all_ok"].as_bool().unwrap_or(false),
        "extract": probes.extract_report["all_ok"].as_bool().unwrap_or(false),
        "embed": probes.embed_report["all_ok"].as_bool().unwrap_or(false),
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
    openai_state_pair: (&'static str, bool),
    openai_model: &str,
) -> Value {
    let tei_info = probes.tei_info_probe.0.clone();
    let tei_info_detail = probes.tei_info_probe.1.clone();
    let tei_model = tei_info.as_ref().and_then(tei_model_from_info);
    let tei_summary = tei_info.as_ref().and_then(tei_info_summary);

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
        "openai": {
            "ok": openai_state_pair.1,
            "state": openai_state_pair.0,
            "base_url": cfg.openai_base_url,
            "model": openai_model,
        },
    })
}

fn report_overall_ok(pipelines: &Value, tei_ok: bool, qdrant_ok: bool) -> bool {
    pipelines["crawl"].as_bool().unwrap_or(false)
        && pipelines["batch"].as_bool().unwrap_or(false)
        && pipelines["extract"].as_bool().unwrap_or(false)
        && pipelines["embed"].as_bool().unwrap_or(false)
        && tei_ok
        && qdrant_ok
}

// NOTE: run_doctor delegates to build_doctor_report and only renders output.
// Keep probe logic centralized in build_doctor_report to avoid drift.
pub async fn build_doctor_report(cfg: &Config) -> Result<Value, Box<dyn Error>> {
    let webdriver_url = webdriver_url_from_env();
    let diagnostics = browser_diagnostics_pattern();
    let probes = gather_doctor_probes(cfg, webdriver_url.as_deref()).await?;
    let pipelines = build_pipeline_status(&probes);
    let openai_model = resolve_openai_model(cfg);
    let openai = openai_state(cfg, &openai_model);
    let services = build_services_status(
        cfg,
        &probes,
        webdriver_url.as_deref(),
        openai,
        &openai_model,
    );

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

    Ok(serde_json::json!({
        "services": services,
        "pipelines": pipelines,
        "browser_runtime": browser_runtime,
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

fn report_bool(report: &serde_json::Value, path: &[&str]) -> bool {
    path.iter()
        .fold(report, |curr, key| {
            curr.get(*key).unwrap_or(&serde_json::Value::Null)
        })
        .as_bool()
        .unwrap_or(false)
}

fn report_text(report: &serde_json::Value, path: &[&str], default: &str) -> String {
    path.iter()
        .fold(report, |curr, key| {
            curr.get(*key).unwrap_or(&serde_json::Value::Null)
        })
        .as_str()
        .unwrap_or(default)
        .to_string()
}

fn status_from_bool(ok: bool) -> &'static str {
    if ok {
        "completed"
    } else {
        "failed"
    }
}

fn render_status_line(name: &str, ok: bool, detail: &str) {
    let status = status_from_bool(ok);
    println!(
        "  {} {} {} {}",
        symbol_for_status(status),
        name,
        status_text(status),
        muted(detail),
    );
}

fn render_tei_info_lines(report: &serde_json::Value) {
    if let Some(model) = report["services"]["tei"]["model"].as_str() {
        println!("    model: {}", muted(model));
    }
    if let Some(summary) = report["services"]["tei"]["summary"].as_str() {
        println!("    info: {}", muted(summary));
    } else if let Some(detail) = report["services"]["tei"]["info_detail"].as_str() {
        println!("    info: {}", muted(detail));
    }
}

fn webdriver_status_label(report: &serde_json::Value) -> String {
    if report_bool(report, &["services", "webdriver", "configured"]) {
        format!(
            "{} ({})",
            redact_url(&report_text(report, &["services", "webdriver", "url"], "")),
            report_text(report, &["services", "webdriver", "detail"], "unreachable")
        )
    } else {
        "not configured (optional fallback)".to_string()
    }
}

fn render_services_section(report: &serde_json::Value) {
    println!("{}", primary("Services"));

    let postgres_ok = report_bool(report, &["services", "postgres", "ok"]);
    let redis_ok = report_bool(report, &["services", "redis", "ok"]);
    let amqp_ok = report_bool(report, &["services", "amqp", "ok"]);
    let tei_ok = report_bool(report, &["services", "tei", "ok"]);
    let qdrant_ok = report_bool(report, &["services", "qdrant", "ok"]);
    let webdriver_ok = report_bool(report, &["services", "webdriver", "ok"]);
    let openai_ok = report_bool(report, &["services", "openai", "ok"]);

    render_status_line(
        "postgres",
        postgres_ok,
        &report_text(report, &["services", "postgres", "url"], "n/a"),
    );
    render_status_line(
        "redis",
        redis_ok,
        &report_text(report, &["services", "redis", "url"], "n/a"),
    );
    render_status_line(
        "amqp",
        amqp_ok,
        &report_text(report, &["services", "amqp", "url"], "n/a"),
    );
    render_status_line(
        "tei",
        tei_ok,
        &report_text(report, &["services", "tei", "detail"], "unreachable"),
    );
    render_tei_info_lines(report);
    render_status_line(
        "qdrant",
        qdrant_ok,
        &report_text(report, &["services", "qdrant", "detail"], "unreachable"),
    );
    render_status_line("webdriver", webdriver_ok, &webdriver_status_label(report));
    render_status_line(
        "openai",
        openai_ok,
        &report_text(report, &["services", "openai", "state"], "not configured"),
    );
}

fn render_pipelines_section(report: &serde_json::Value) {
    println!("{}", primary("Pipelines"));
    for name in ["crawl", "batch", "extract", "embed"] {
        let ok = report_bool(report, &["pipelines", name]);
        let status = status_from_bool(ok);
        println!(
            "  {} {} {}",
            symbol_for_status(status),
            name,
            status_text(status),
        );
    }
}

fn diagnostics_enabled_label(report: &serde_json::Value) -> &'static str {
    if report_bool(report, &["browser_runtime", "diagnostics", "enabled"]) {
        "enabled"
    } else {
        "disabled"
    }
}

fn render_browser_runtime_section(report: &serde_json::Value) {
    println!("{}", primary("Browser Runtime"));
    println!(
        "  selection: {}",
        muted(&report_text(
            report,
            &["browser_runtime", "selection"],
            "unknown"
        ))
    );
    println!(
        "  diagnostics: {} (screenshot={} events={} dir={})",
        muted(diagnostics_enabled_label(report)),
        report_bool(report, &["browser_runtime", "diagnostics", "screenshot"]),
        report_bool(report, &["browser_runtime", "diagnostics", "events"]),
        report_text(
            report,
            &["browser_runtime", "diagnostics", "output_dir"],
            "."
        ),
    );
}

fn render_doctor_report_human(report: &serde_json::Value) {
    let all_ok = report_bool(report, &["all_ok"]);

    println!("{}", primary("Doctor Report"));
    println!();
    render_services_section(report);

    println!();
    render_pipelines_section(report);

    println!();
    render_browser_runtime_section(report);

    println!();
    let status = status_from_bool(all_ok);
    println!(
        "{} overall {}",
        symbol_for_status(status),
        status_text(status),
    );
}
