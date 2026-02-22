use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::extract_jobs::{start_extract_job, start_extract_job_with_pool};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::error::Error;
use std::fmt::Write;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionCandidate {
    pub url: String,
    pub markdown_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInjectionRule {
    pub name: String,
    pub min_markdown_chars: usize,
    pub min_quality_score: f64,
    pub max_urls: usize,
    pub url_contains_any: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInjectionDecision {
    pub url: String,
    pub markdown_chars: usize,
    pub quality_score: f64,
    pub selected: bool,
    pub matched_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSelectionStats {
    pub name: String,
    pub selected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionObservability {
    pub input_tokens_estimated: u64,
    pub output_tokens_estimated: u64,
    pub total_tokens_estimated: u64,
    pub estimated_cost_usd: f64,
    pub avg_quality_score: f64,
    pub quality_band: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInjectionEvaluation {
    pub phase: String,
    pub total_candidates: usize,
    pub selected_candidates: usize,
    pub rules: Vec<QueueInjectionRule>,
    pub selected_urls: Vec<String>,
    pub selected_by_rule: Vec<RuleSelectionStats>,
    pub decisions: Vec<QueueInjectionDecision>,
    pub observability: ExtractionObservability,
}

fn default_queue_injection_rules() -> Vec<QueueInjectionRule> {
    vec![
        QueueInjectionRule {
            name: "docs-first".to_string(),
            min_markdown_chars: 800,
            min_quality_score: 0.55,
            max_urls: 12,
            url_contains_any: vec![
                "docs".to_string(),
                "api".to_string(),
                "reference".to_string(),
                "guide".to_string(),
            ],
        },
        QueueInjectionRule {
            name: "tutorial-longform".to_string(),
            min_markdown_chars: 1600,
            min_quality_score: 0.60,
            max_urls: 8,
            url_contains_any: vec![
                "tutorial".to_string(),
                "blog".to_string(),
                "article".to_string(),
                "learn".to_string(),
            ],
        },
        QueueInjectionRule {
            name: "high-signal-catchall".to_string(),
            min_markdown_chars: 2200,
            min_quality_score: 0.72,
            max_urls: 4,
            url_contains_any: vec![],
        },
    ]
}

fn load_queue_injection_rules() -> Vec<QueueInjectionRule> {
    let maybe_rules = match std::env::var("AXON_QUEUE_INJECTION_RULES_JSON") {
        Ok(raw) => match serde_json::from_str::<Vec<QueueInjectionRule>>(&raw) {
            Ok(rules) if !rules.is_empty() => Some(rules),
            Ok(_) => None,
            Err(e) => {
                log_warn(&format!(
                    "AXON_QUEUE_INJECTION_RULES_JSON parse failed, using defaults: {e}"
                ));
                None
            }
        },
        Err(_) => None,
    };
    maybe_rules.unwrap_or_else(default_queue_injection_rules)
}

fn estimate_quality_score(url: &str, markdown_chars: usize) -> f64 {
    let normalized_url = url.to_ascii_lowercase();
    let density_score = (markdown_chars as f64 / 3500.0).clamp(0.0, 1.0);
    let signal_bonus = if normalized_url.contains("docs")
        || normalized_url.contains("api")
        || normalized_url.contains("reference")
        || normalized_url.contains("guide")
        || normalized_url.contains("tutorial")
    {
        0.20
    } else {
        0.05
    };
    let path_slashes = normalized_url
        .find("://")
        .and_then(|i| normalized_url.get(i + 3..))
        .map(|path| path.matches('/').count())
        .unwrap_or(0);
    let depth_bonus = (path_slashes as f64 / 12.0).clamp(0.0, 0.10);
    (0.70 * density_score + signal_bonus + depth_bonus).clamp(0.0, 1.0)
}

fn quality_band(score: f64) -> String {
    if score >= 0.80 {
        "high".to_string()
    } else if score >= 0.55 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn estimate_input_tokens(markdown_chars: usize) -> u64 {
    ((markdown_chars as f64 / 4.0).ceil() as u64).max(64)
}

pub fn evaluate_queue_injection(
    candidates: &[InjectionCandidate],
    phase: &str,
) -> QueueInjectionEvaluation {
    let rules = load_queue_injection_rules();
    let mut sorted = candidates.to_vec();
    sorted.sort_by(|a, b| b.markdown_chars.cmp(&a.markdown_chars));

    let mut selected_urls = Vec::new();
    let mut decisions = Vec::new();
    let mut selected_by_rule: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();

    for candidate in sorted {
        let quality = estimate_quality_score(&candidate.url, candidate.markdown_chars);
        let candidate_url = candidate.url.to_ascii_lowercase();
        let mut matched_rule = None;

        for rule in &rules {
            if candidate.markdown_chars < rule.min_markdown_chars {
                continue;
            }
            if quality < rule.min_quality_score {
                continue;
            }

            let keyword_match = rule.url_contains_any.is_empty()
                || rule
                    .url_contains_any
                    .iter()
                    .any(|k| candidate_url.contains(&k.to_ascii_lowercase()));
            if !keyword_match {
                continue;
            }

            let already_selected = selected_by_rule.get(&rule.name).copied().unwrap_or(0);
            if already_selected >= rule.max_urls {
                continue;
            }

            *selected_by_rule.entry(rule.name.clone()).or_insert(0) += 1;
            selected_urls.push(candidate.url.clone());
            matched_rule = Some(rule.name.clone());
            break;
        }

        decisions.push(QueueInjectionDecision {
            url: candidate.url,
            markdown_chars: candidate.markdown_chars,
            quality_score: quality,
            selected: matched_rule.is_some(),
            matched_rule,
        });
    }

    let input_tokens_estimated: u64 = decisions
        .iter()
        .filter(|d| d.selected)
        .map(|d| estimate_input_tokens(d.markdown_chars))
        .sum();
    let output_tokens_estimated = (selected_urls.len() as u64).saturating_mul(220);
    let total_tokens_estimated = input_tokens_estimated.saturating_add(output_tokens_estimated);

    let price_per_1k_tokens = std::env::var("AXON_EXTRACT_EST_COST_PER_1K_TOKENS")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(0.0025);
    let estimated_cost_usd =
        ((total_tokens_estimated as f64 / 1000.0) * price_per_1k_tokens * 100_000.0).round()
            / 100_000.0;

    let selected_quality_total: f64 = decisions
        .iter()
        .filter(|d| d.selected)
        .map(|d| d.quality_score)
        .sum();
    let avg_quality_score = if selected_urls.is_empty() {
        0.0
    } else {
        selected_quality_total / selected_urls.len() as f64
    };

    let selected_by_rule = selected_by_rule
        .into_iter()
        .map(|(name, selected)| RuleSelectionStats { name, selected })
        .collect();

    QueueInjectionEvaluation {
        phase: phase.to_string(),
        total_candidates: decisions.len(),
        selected_candidates: selected_urls.len(),
        rules,
        selected_urls,
        selected_by_rule,
        decisions,
        observability: ExtractionObservability {
            input_tokens_estimated,
            output_tokens_estimated,
            total_tokens_estimated,
            estimated_cost_usd,
            avg_quality_score,
            quality_band: quality_band(avg_quality_score),
        },
    }
}

pub async fn apply_queue_injection(
    cfg: &Config,
    candidates: &[InjectionCandidate],
    extraction_prompt: Option<&str>,
    phase: &str,
    enqueue_enabled: bool,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let evaluation = evaluate_queue_injection(candidates, phase);
    let mut payload = serde_json::to_value(&evaluation)?;
    let selected_urls = evaluation.selected_urls.clone();
    let prompt = extraction_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let mut queue_status = String::new();
    let mut extract_job_id: Option<Uuid> = None;

    if selected_urls.is_empty() {
        queue_status.push_str("skipped_no_candidates");
    } else if prompt.is_none() {
        queue_status.push_str("skipped_missing_prompt");
    } else if !enqueue_enabled {
        queue_status.push_str("deferred");
    } else {
        let job_id = start_extract_job(cfg, &selected_urls, prompt).await?;
        write!(&mut queue_status, "enqueued").ok();
        extract_job_id = Some(job_id);
        log_info(&format!(
            "command=queue_injection phase={} selected={} extract_job_id={}",
            phase,
            selected_urls.len(),
            job_id
        ));
    }

    if let Some(object) = payload.as_object_mut() {
        object.insert("queue_status".to_string(), serde_json::json!(queue_status));
        object.insert(
            "enqueue_enabled".to_string(),
            serde_json::json!(enqueue_enabled),
        );
        object.insert(
            "extract_job_id".to_string(),
            serde_json::json!(extract_job_id),
        );
    }

    Ok(payload)
}

/// Pool-aware variant of `apply_queue_injection` for use inside workers that
/// already hold a long-lived `PgPool`. Avoids creating a new connection pool
/// for the extract-job enqueue step.
pub(crate) async fn apply_queue_injection_with_pool(
    pool: &PgPool,
    cfg: &Config,
    candidates: &[InjectionCandidate],
    extraction_prompt: Option<&str>,
    phase: &str,
    enqueue_enabled: bool,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let evaluation = evaluate_queue_injection(candidates, phase);
    let mut payload = serde_json::to_value(&evaluation)?;
    let selected_urls = evaluation.selected_urls.clone();
    let prompt = extraction_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let mut queue_status = String::new();
    let mut extract_job_id: Option<Uuid> = None;

    if selected_urls.is_empty() {
        queue_status.push_str("skipped_no_candidates");
    } else if prompt.is_none() {
        queue_status.push_str("skipped_missing_prompt");
    } else if !enqueue_enabled {
        queue_status.push_str("deferred");
    } else {
        let job_id = start_extract_job_with_pool(pool, cfg, &selected_urls, prompt).await?;
        queue_status.push_str("enqueued");
        extract_job_id = Some(job_id);
        log_info(&format!(
            "command=queue_injection phase={} selected={} extract_job_id={}",
            phase,
            selected_urls.len(),
            job_id
        ));
    }

    if let Some(object) = payload.as_object_mut() {
        object.insert("queue_status".to_string(), serde_json::json!(queue_status));
        object.insert(
            "enqueue_enabled".to_string(),
            serde_json::json!(enqueue_enabled),
        );
        object.insert(
            "extract_job_id".to_string(),
            serde_json::json!(extract_job_id),
        );
    }

    Ok(payload)
}
