pub(super) struct NormalizedExcludePrefixes {
    pub(super) prefixes: Vec<String>,
    pub(super) disable_defaults: bool,
}

pub(super) fn default_exclude_prefixes() -> Vec<String> {
    vec![
        "/fr", "/de", "/es", "/ja", "/zh", "/zh-cn", "/zh-tw", "/ko", "/pt", "/pt-br", "/it",
        "/nl", "/pl", "/ru", "/tr", "/ar", "/id", "/vi", "/th", "/cs", "/da", "/fi", "/no", "/sv",
        "/he", "/uk", "/ro", "/hu", "/el",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(super) fn normalize_exclude_prefixes(input: Vec<String>) -> NormalizedExcludePrefixes {
    let disable_by_empty = input.iter().any(|v| matches!(v.trim(), "" | "/"));
    let disable_by_none = input.iter().any(|v| v.trim().eq_ignore_ascii_case("none"));
    if disable_by_none {
        let ignored: Vec<&str> = input
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.eq_ignore_ascii_case("none"))
            .filter(|value| !value.is_empty() && *value != "/")
            .collect();
        if !ignored.is_empty() {
            eprintln!(
                "warning: --exclude-path-prefix 'none' disables exclusions; ignoring additional prefixes: {}",
                ignored.join(", ")
            );
        }
        return NormalizedExcludePrefixes {
            prefixes: Vec::new(),
            disable_defaults: true,
        };
    }

    let mut out = Vec::new();
    for raw in input {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "/" {
            continue;
        }
        let normalized = if trimmed.starts_with('/') {
            trimmed.to_string()
        } else {
            format!("/{trimmed}")
        };
        out.push(normalized);
    }
    out.sort();
    out.dedup();
    NormalizedExcludePrefixes {
        prefixes: out,
        disable_defaults: disable_by_empty,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_exclude_prefixes;

    #[test]
    fn normalize_exclude_prefixes_none_disables_defaults() {
        let normalized = normalize_exclude_prefixes(vec!["none".to_string()]);
        assert!(normalized.disable_defaults);
        assert!(normalized.prefixes.is_empty());
    }

    #[test]
    fn normalize_exclude_prefixes_none_with_values_still_disables() {
        let normalized = normalize_exclude_prefixes(vec!["none".to_string(), "/fr".to_string()]);
        assert!(normalized.disable_defaults);
        assert!(normalized.prefixes.is_empty());
    }
}
