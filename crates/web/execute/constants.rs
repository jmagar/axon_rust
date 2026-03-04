/// WebSocket allowed execution modes.
/// IMPORTANT: This list MUST stay in sync with apps/web/lib/ws-protocol.ts (MODES constant).
/// When adding a mode here, also add it to the TypeScript MODES array in that file.
/// See docs/WS-PROTOCOL.md for the full protocol contract.
pub(super) const ALLOWED_MODES: &[&str] = &[
    "scrape",
    "crawl",
    "map",
    "extract",
    "search",
    "research",
    "embed",
    "debug",
    "doctor",
    "query",
    "retrieve",
    "ask",
    "evaluate",
    "suggest",
    "sources",
    "domains",
    "stats",
    "status",
    "dedupe",
    "github",
    "reddit",
    "youtube",
    "sessions",
    "screenshot",
];

pub(super) const ALLOWED_FLAGS: &[(&str, &str)] = &[
    ("max_pages", "--max-pages"),
    ("max_depth", "--max-depth"),
    ("limit", "--limit"),
    ("collection", "--collection"),
    ("format", "--format"),
    ("render_mode", "--render-mode"),
    ("include_subdomains", "--include-subdomains"),
    ("discover_sitemaps", "--discover-sitemaps"),
    ("sitemap_since_days", "--sitemap-since-days"),
    ("embed", "--embed"),
    ("diagnostics", "--diagnostics"),
    ("yes", "--yes"),
    ("wait", "--wait"),
    ("research_depth", "--research-depth"),
    ("search_time_range", "--search-time-range"),
    ("sort", "--sort"),
    ("time", "--time"),
    ("max_posts", "--max-posts"),
    ("min_score", "--min-score"),
    ("scrape_links", "--scrape-links"),
    ("include_source", "--include-source"),
    ("claude", "--claude"),
    ("codex", "--codex"),
    ("gemini", "--gemini"),
    ("project", "--project"),
    ("output_dir", "--output-dir"),
    ("delay_ms", "--delay-ms"),
    ("request_timeout_ms", "--request-timeout-ms"),
    ("performance_profile", "--performance-profile"),
    ("batch_concurrency", "--batch-concurrency"),
    ("depth", "--depth"),
    ("responses_mode", "--responses-mode"),
];

pub(super) const ASYNC_MODES: &[&str] =
    &["crawl", "extract", "embed", "github", "reddit", "youtube"];

/// Commands that produce streaming/non-JSON output and must NOT receive --json.
/// When adding a new command, the default is to receive --json. Add here only if
/// the command's output format is inherently non-JSON (e.g., streaming text synthesis).
pub(super) const NO_JSON_MODES: &[&str] = &[
    "search", // Tavily streaming output — results are printed as plain text, not structured JSON
    "research", // Spider agent streaming synthesis — narrative text output, not structured JSON
];
