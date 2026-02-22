use super::*;

#[test]
fn test_redact_url_postgres() {
    let url = "postgresql://axon:secret123@localhost:5432/axon";
    let redacted = redact_url(url);
    assert!(!redacted.contains("secret123"));
    assert!(redacted.contains("***"));
}

#[test]
fn test_redact_url_amqp() {
    let url = "amqp://guest:guest@localhost:5672";
    let redacted = redact_url(url);
    assert!(!redacted.contains("guest:guest"));
}

#[test]
fn test_redact_url_no_credentials() {
    let url = "http://example.com/path";
    assert_eq!(redact_url(url), url);
}

#[test]
fn test_redact_url_unparseable() {
    let result = redact_url("not a url at all !!!@#$");
    assert_eq!(result, "***redacted***");
}

#[test]
fn test_redact_url_username_only() {
    let url = "postgresql://admin@localhost:5432/db";
    let redacted = redact_url(url);
    assert!(!redacted.contains("admin@"));
    assert!(redacted.contains("***"));
}

#[test]
fn test_redact_url_redis_with_password() {
    let url = "redis://:mypassword@localhost:6379";
    let redacted = redact_url(url);
    assert!(!redacted.contains("mypassword"));
}

#[test]
fn test_default_engine_extracts_json_ld() {
    let html = r#"
        <html><head>
        <script type="application/ld+json">{"@type":"Article","headline":"Hello"}</script>
        </head></html>
    "#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert!(!page.items.is_empty());
    assert!(page.parser_hits.iter().any(|x| x == "json-ld"));
}

#[test]
fn test_default_engine_dedups_identical_json_ld_items() {
    let html = r#"
        <html><head>
        <script type="application/ld+json">{"@type":"Article","headline":"Hello"}</script>
        <script type="application/ld+json">{"@type":"Article","headline":"Hello"}</script>
        </head></html>
    "#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert_eq!(page.items.len(), 1);
}

#[test]
fn test_extract_attr_case_insensitive() {
    let tag = r#"<meta PROPERTY = "og:title" content="Example">"#;
    assert_eq!(
        deterministic::extract_attr(tag, "property").as_deref(),
        Some("og:title")
    );
}

#[test]
fn test_estimate_llm_cost_usd_zero_for_unknown_model() {
    let cost = deterministic::estimate_llm_cost_usd("unknown-model", 10_000, 1_000);
    assert_eq!(cost, 0.0);
}

#[test]
fn test_estimate_llm_cost_usd_known_model() {
    let cost = deterministic::estimate_llm_cost_usd("gpt-4o-mini", 100_000, 20_000);
    assert!(cost > 0.0);
}

// --- OpenGraphParser tests ---

#[test]
fn open_graph_parser_extracts_title() {
    let html = r#"<html><head>
        <meta property="og:title" content="My Page Title">
    </head></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert!(page.parser_hits.iter().any(|h| h == "open-graph"));
    let item = page.items.iter().find(|v| v.get("og:title").is_some());
    assert!(item.is_some(), "og:title field should be present");
    assert_eq!(item.unwrap()["og:title"].as_str(), Some("My Page Title"));
}

#[test]
fn open_graph_parser_extracts_description() {
    let html = r#"<html><head>
        <meta property="og:description" content="A page about testing">
    </head></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert!(page.parser_hits.iter().any(|h| h == "open-graph"));
    let item = page
        .items
        .iter()
        .find(|v| v.get("og:description").is_some());
    assert!(item.is_some(), "og:description field should be present");
    assert_eq!(
        item.unwrap()["og:description"].as_str(),
        Some("A page about testing")
    );
}

#[test]
fn open_graph_parser_returns_empty_for_no_og_tags() {
    let html = r#"<html><head>
        <meta name="description" content="Not an OG tag">
    </head></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert!(!page.parser_hits.iter().any(|h| h == "open-graph"));
}

#[test]
fn open_graph_parser_injects_source_url_and_parser_fields() {
    let html = r#"<html><head>
        <meta property="og:title" content="Test">
    </head></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com/page", html);
    let item = page
        .items
        .iter()
        .find(|v| v.get("og:title").is_some())
        .unwrap();
    assert_eq!(
        item["_source_url"].as_str(),
        Some("https://example.com/page")
    );
    assert_eq!(item["_parser"].as_str(), Some("open-graph"));
}

// --- HtmlTableParser tests ---

#[test]
fn html_table_parser_detects_table_and_counts_rows() {
    let html = r#"<html><body>
        <table>
            <tr><th>Name</th><th>Value</th></tr>
            <tr><td>foo</td><td>1</td></tr>
            <tr><td>bar</td><td>2</td></tr>
        </table>
    </body></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert!(page.parser_hits.iter().any(|h| h == "html-table"));
    let item = page.items.iter().find(|v| v.get("rows").is_some()).unwrap();
    assert_eq!(item["rows"].as_u64(), Some(3));
}

#[test]
fn html_table_parser_handles_multiple_tables() {
    let html = r#"<html><body>
        <table><tr><td>A</td></tr></table>
        <table><tr><td>B</td></tr><tr><td>C</td></tr></table>
    </body></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert_eq!(
        page.items
            .iter()
            .filter(|v| v.get("rows").is_some())
            .count(),
        2,
        "should produce one item per table"
    );
}

#[test]
fn html_table_parser_returns_empty_for_no_tables() {
    let html = r#"<html><body><p>No tables here.</p></body></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert!(!page.parser_hits.iter().any(|h| h == "html-table"));
}

#[test]
fn html_table_parser_injects_source_url_and_parser_fields() {
    let html = r#"<table><tr><td>data</td></tr></table>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com/data", html);
    let item = page.items.iter().find(|v| v.get("rows").is_some()).unwrap();
    assert_eq!(
        item["_source_url"].as_str(),
        Some("https://example.com/data")
    );
    assert_eq!(item["_parser"].as_str(), Some("html-table"));
}

// --- DeterministicExtractionEngine general tests ---

#[test]
fn deterministic_engine_handles_no_content() {
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", "");
    assert!(page.items.is_empty());
    assert!(page.parser_hits.is_empty());
}

#[test]
fn deterministic_engine_handles_html_with_no_structured_data() {
    let html = r#"<html><body><h1>Hello</h1><p>Some plain text.</p></body></html>"#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    assert!(page.items.is_empty());
}

#[test]
fn deterministic_engine_deduplicates_identical_json_ld_across_scripts() {
    let html = r#"
        <html><head>
        <script type="application/ld+json">{"@type":"Product","name":"Widget"}</script>
        <script type="application/ld+json">{"@type":"Product","name":"Widget"}</script>
        </head></html>
    "#;
    let engine = DeterministicExtractionEngine::with_default_parsers();
    let page = engine.extract("https://example.com", html);
    // Both json-ld scripts are identical — dedup should reduce to 1 item.
    let product_items: Vec<_> = page
        .items
        .iter()
        .filter(|v| v.get("@type").map(|t| t == "Product").unwrap_or(false))
        .collect();
    assert_eq!(
        product_items.len(),
        1,
        "identical items must be deduplicated"
    );
}

// --- canonicalize_url tests ---

#[test]
fn canonicalize_url_strips_default_http_port() {
    assert_eq!(
        canonicalize_url("http://example.com:80/path"),
        Some("http://example.com/path".to_string())
    );
}

#[test]
fn canonicalize_url_strips_default_https_port() {
    assert_eq!(
        canonicalize_url("https://example.com:443/page/"),
        Some("https://example.com/page".to_string())
    );
}

#[test]
fn canonicalize_url_keeps_non_default_port() {
    assert_eq!(
        canonicalize_url("https://example.com:8443/path"),
        Some("https://example.com:8443/path".to_string())
    );
}

#[test]
fn canonicalize_url_strips_fragment_and_trailing_slash() {
    assert_eq!(
        canonicalize_url("https://example.com/docs/#section"),
        Some("https://example.com/docs".to_string())
    );
}
