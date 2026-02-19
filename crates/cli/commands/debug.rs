use crate::axon_cli::crates::cli::commands::doctor::build_doctor_report;
use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::http::http_client;
use crate::axon_cli::crates::core::ui::{muted, primary};
use serde_json::Value;
use std::env;
use std::error::Error;

fn resolve_openai_model(cfg: &Config) -> String {
    if !cfg.openai_model.trim().is_empty() {
        return cfg.openai_model.clone();
    }
    env::var("OPENAI_MODEL").unwrap_or_default()
}

fn bool_field(value: &Value, path: &[&str]) -> bool {
    let mut current = value;
    for key in path {
        current = current.get(*key).unwrap_or(&Value::Null);
    }
    current.as_bool().unwrap_or(false)
}

fn string_field<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    let mut current = value;
    for key in path {
        current = current.get(*key).unwrap_or(&Value::Null);
    }
    current.as_str().unwrap_or("")
}

pub async fn run_debug(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let doctor_report = build_doctor_report(cfg).await?;

    let openai_base_url = cfg.openai_base_url.trim().trim_end_matches('/').to_string();
    let openai_model = resolve_openai_model(cfg);
    if openai_base_url.is_empty() {
        return Err("OPENAI_BASE_URL is required for debug".into());
    }
    if openai_model.is_empty() {
        return Err("OPENAI_MODEL is required for debug".into());
    }

    let user_context = if cfg.positional.is_empty() {
        String::new()
    } else {
        cfg.positional.join(" ")
    };

    let prompt = format!(
        "Analyze this Axon doctor report and provide actionable troubleshooting guidance.\n\
         Prioritize root causes and concrete fix commands.\n\
         Keep it concise and operator-friendly.\n\
         Include:\n\
         1) likely root causes ordered by confidence\n\
         2) exact verification commands\n\
         3) exact remediation commands\n\
         4) what to check next if fixes fail\n\n\
         Optional operator context:\n{}\n\n\
         Doctor report JSON:\n{}",
        if user_context.is_empty() {
            "(none)"
        } else {
            &user_context
        },
        serde_json::to_string_pretty(&doctor_report)?
    );

    let client = http_client()?;
    let mut req = client
        .post(format!("{openai_base_url}/chat/completions"))
        .json(&serde_json::json!({
            "model": openai_model,
            "messages": [
                {"role": "system", "content": "You are a senior self-hosted infrastructure debugging assistant. Be precise and avoid generic advice."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.1
        }));

    if !cfg.openai_api_key.trim().is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let response = req.send().await?.error_for_status()?;
    let body: Value = response.json().await?;
    let analysis = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no debug response)");

    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "doctor_report": doctor_report,
                "llm_debug": {
                    "model": resolve_openai_model(cfg),
                    "base_url": cfg.openai_base_url,
                    "analysis": analysis,
                }
            }))?
        );
        return Ok(());
    }

    println!("{}", primary("Debug Snapshot"));
    println!(
        "  {} {}",
        muted("overall:"),
        if bool_field(&doctor_report, &["all_ok"]) {
            "healthy"
        } else {
            "degraded"
        }
    );
    println!(
        "  {} {}",
        muted("tei:"),
        string_field(&doctor_report, &["services", "tei", "model"])
    );
    println!(
        "  {} {}",
        muted("openai model:"),
        string_field(&doctor_report, &["services", "openai", "model"])
    );
    println!();
    println!("{}", primary("LLM Debug"));
    println!("{analysis}");

    Ok(())
}
