pub(super) struct NormalizedExcludePrefixes {
    pub(super) prefixes: Vec<String>,
    pub(super) disable_defaults: bool,
}

pub fn default_exclude_prefixes() -> Vec<String> {
    vec![
        // Auth / account flows -- no indexable content
        "/auth",
        "/login",
        "/logout",
        "/register",
        "/signin",
        "/signup",
        // Legal / compliance boilerplate
        "/cookie-policy",
        "/cookies",
        "/legal",
        "/privacy",
        "/terms",
        // CDN and framework internals -- never user-facing content
        "/_astro",
        "/_next",
        "/_nuxt",
        "/_vercel",
        "/__nextjs",
        "/cdn-cgi",
        "/wp-admin",
        "/wp-includes",
        // Syndication feeds -- XML, not useful for RAG
        "/atom",
        "/feed",
        "/rss",
        // Duplicate / utility page variants
        "/print",
        "/search",
        // Forum / community discussion threads
        "/answers",
        "/discussions",
        "/forum",
        "/forums",
        "/questions",
        // Non-English locales
        "/ar",
        "/cs",
        "/da",
        "/de",
        "/el",
        "/es",
        "/fi",
        "/fr",
        "/he",
        "/hu",
        "/id",
        "/it",
        "/ja",
        "/ko",
        "/nl",
        "/no",
        "/pl",
        "/pt",
        "/pt-br",
        "/ro",
        "/ru",
        "/sv",
        "/th",
        "/tr",
        "/uk",
        "/vi",
        "/zh",
        "/zh-cn",
        "/zh-tw",
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
