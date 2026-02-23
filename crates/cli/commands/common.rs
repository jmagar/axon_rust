use crate::crates::core::config::{CommandKind, Config};
use crate::crates::core::http::normalize_url;
use std::collections::HashSet;

fn expand_numeric_range(start: i64, end: i64, step: i64) -> Vec<String> {
    let mut out = Vec::new();
    if step == 0 {
        return out;
    }
    let mut current = start;
    if start <= end && step > 0 {
        while current <= end {
            out.push(current.to_string());
            current += step;
        }
    } else if start >= end && step < 0 {
        while current >= end {
            out.push(current.to_string());
            current += step;
        }
    }
    out
}

fn expand_brace_token(token: &str) -> Vec<String> {
    let trimmed = token.trim();
    if let Some((lhs, rhs)) = trimmed.split_once("..") {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        if let (Ok(start), Ok(end)) = (lhs.parse::<i64>(), rhs.parse::<i64>()) {
            let step = if start <= end { 1 } else { -1 };
            let values = expand_numeric_range(start, end, step);
            if !values.is_empty() {
                return values;
            }
        }
    }
    trimmed
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

const MAX_EXPANSION_DEPTH: usize = 10;

fn expand_url_glob_seed(seed: &str) -> Vec<String> {
    expand_url_glob_seed_inner(seed, 0)
}

fn expand_url_glob_seed_inner(seed: &str, depth: usize) -> Vec<String> {
    if depth >= MAX_EXPANSION_DEPTH {
        return vec![seed.to_string()];
    }
    let Some(open_idx) = seed.find('{') else {
        return vec![seed.to_string()];
    };
    let Some(close_rel) = seed[open_idx + 1..].find('}') else {
        return vec![seed.to_string()];
    };
    let close_idx = open_idx + 1 + close_rel;
    let prefix = &seed[..open_idx];
    let token = &seed[open_idx + 1..close_idx];
    let suffix = &seed[close_idx + 1..];
    let choices = expand_brace_token(token);
    if choices.is_empty() {
        return vec![seed.to_string()];
    }

    let mut out = Vec::new();
    for choice in choices {
        let next = format!("{prefix}{choice}{suffix}");
        out.extend(expand_url_glob_seed_inner(&next, depth + 1));
    }
    out
}

pub fn parse_urls(cfg: &Config) -> Vec<String> {
    let mut out = Vec::new();
    let mut raw = Vec::new();
    if let Some(csv) = &cfg.urls_csv {
        raw.extend(
            csv.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        );
    }
    raw.extend(
        cfg.url_glob
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    );
    raw.extend(
        cfg.positional
            .iter()
            .flat_map(|s| s.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    );
    let mut seen = HashSet::new();
    for seed in raw {
        for expanded in expand_url_glob_seed(&seed) {
            let normalized = normalize_url(&expanded);
            if seen.insert(normalized.clone()) {
                out.push(normalized);
            }
        }
    }
    out
}

pub fn start_url_from_cfg(cfg: &Config) -> String {
    if matches!(
        cfg.command,
        CommandKind::Crawl | CommandKind::Extract | CommandKind::Embed
    ) && matches!(
        cfg.positional.first().map(|s| s.as_str()),
        Some("status" | "cancel" | "errors" | "list" | "cleanup" | "clear" | "worker" | "doctor")
    ) {
        return cfg.start_url.clone();
    }

    if matches!(
        cfg.command,
        CommandKind::Scrape
            | CommandKind::Map
            | CommandKind::Crawl
            | CommandKind::Extract
            | CommandKind::Embed
    ) {
        let selected = cfg
            .positional
            .first()
            .cloned()
            .unwrap_or_else(|| cfg.start_url.clone());
        return normalize_url(&selected);
    }

    cfg.start_url.clone()
}

#[cfg(test)]
mod tests {
    use super::expand_url_glob_seed;

    #[test]
    fn expands_url_glob_range() {
        let expanded = expand_url_glob_seed("https://example.com/page/{1..3}");
        assert_eq!(
            expanded,
            vec![
                "https://example.com/page/1".to_string(),
                "https://example.com/page/2".to_string(),
                "https://example.com/page/3".to_string()
            ]
        );
    }

    #[test]
    fn expands_url_glob_list_and_nested() {
        let expanded = expand_url_glob_seed("https://example.com/{news,docs}/{a,b}");
        assert_eq!(
            expanded,
            vec![
                "https://example.com/news/a".to_string(),
                "https://example.com/news/b".to_string(),
                "https://example.com/docs/a".to_string(),
                "https://example.com/docs/b".to_string()
            ]
        );
    }
}
