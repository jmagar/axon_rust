use crate::axon_cli::crates::core::config::Config;
use std::error::Error;

/// Returns true if a file path should be indexed when --include-source is set.
/// Excludes lock files, generated files, binaries, and non-code files.
pub fn is_indexable_source_path(path: &str) -> bool {
    // Reject build artifact directories
    if path.starts_with("target/")
        || path.contains("/target/")
        || path.starts_with("node_modules/")
        || path.contains("/node_modules/")
        || path.starts_with("dist/")
        || path.contains("/dist/")
        || path.contains("__pycache__")
    {
        return false;
    }

    // Reject lock files by name suffix
    if path.ends_with(".lock") || path.ends_with("-lock.json") || path.ends_with(".lock.json") {
        return false;
    }

    // Accept known source extensions
    let accepted = [".rs", ".py", ".go", ".ts", ".js", ".tsx", ".jsx", ".toml"];
    accepted.iter().any(|ext| path.ends_with(ext))
}

/// Returns true if a file path should always be indexed (markdown/docs), regardless of --include-source.
pub fn is_indexable_doc_path(path: &str) -> bool {
    let accepted = [".md", ".mdx", ".rst", ".txt"];
    accepted.iter().any(|ext| path.ends_with(ext))
}

/// Parse an "owner/repo" string into (owner, repo) parts.
/// Accepts both "owner/repo" and "https://github.com/owner/repo" forms.
pub fn parse_github_repo(input: &str) -> Option<(String, String)> {
    let normalized = if let Some(rest) = input.strip_prefix("https://github.com/") {
        rest.trim_end_matches('/')
    } else {
        input
    };

    let mut parts = normalized.splitn(2, '/');
    let owner = parts.next().filter(|s| !s.is_empty())?;
    let repo = parts.next().filter(|s| !s.is_empty() && !s.contains('/'))?;

    Some((owner.to_string(), repo.to_string()))
}

/// Ingest a GitHub repository:
/// - Fetches markdown files + issues + PRs via GitHub REST API
/// - Optionally fetches source files if include_source is true
/// - Embeds all content into Qdrant via embed_text_with_metadata
pub async fn ingest_github(
    _cfg: &Config,
    _repo: &str,
    _include_source: bool,
) -> Result<usize, Box<dyn Error>> {
    todo!("implement GitHub ingestion")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_indexable_source_path ---

    #[test]
    fn source_path_accepts_rust_files() {
        assert!(is_indexable_source_path("src/main.rs"));
        assert!(is_indexable_source_path("lib/foo.rs"));
    }

    #[test]
    fn source_path_accepts_python_files() {
        assert!(is_indexable_source_path("src/app.py"));
    }

    #[test]
    fn source_path_accepts_typescript_and_js() {
        assert!(is_indexable_source_path("src/index.ts"));
        assert!(is_indexable_source_path("utils/helper.js"));
    }

    #[test]
    fn source_path_accepts_go_files() {
        assert!(is_indexable_source_path("main.go"));
    }

    #[test]
    fn source_path_rejects_lock_files() {
        assert!(!is_indexable_source_path("Cargo.lock"));
        assert!(!is_indexable_source_path("package-lock.json"));
        assert!(!is_indexable_source_path("yarn.lock"));
        assert!(!is_indexable_source_path("Gemfile.lock"));
    }

    #[test]
    fn source_path_rejects_binary_and_image_files() {
        assert!(!is_indexable_source_path("assets/logo.png"));
        assert!(!is_indexable_source_path("icon.svg"));
        assert!(!is_indexable_source_path("font.woff2"));
    }

    #[test]
    fn source_path_rejects_build_artifacts() {
        assert!(!is_indexable_source_path("target/release/axon"));
        assert!(!is_indexable_source_path("dist/bundle.js.map"));
        assert!(!is_indexable_source_path("node_modules/lodash/index.js"));
    }

    // --- is_indexable_doc_path ---

    #[test]
    fn doc_path_accepts_markdown() {
        assert!(is_indexable_doc_path("README.md"));
        assert!(is_indexable_doc_path("docs/guide.md"));
        assert!(is_indexable_doc_path("CONTRIBUTING.md"));
    }

    #[test]
    fn doc_path_rejects_source_code() {
        assert!(!is_indexable_doc_path("src/main.rs"));
    }

    #[test]
    fn doc_path_rejects_lock_files() {
        assert!(!is_indexable_doc_path("Cargo.lock"));
    }

    // --- parse_github_repo ---

    #[test]
    fn parse_repo_from_owner_slash_repo() {
        let result = parse_github_repo("rust-lang/rust");
        assert_eq!(result, Some(("rust-lang".to_string(), "rust".to_string())));
    }

    #[test]
    fn parse_repo_from_github_url() {
        let result = parse_github_repo("https://github.com/rust-lang/rust");
        assert_eq!(result, Some(("rust-lang".to_string(), "rust".to_string())));
    }

    #[test]
    fn parse_repo_from_github_url_with_trailing_slash() {
        let result = parse_github_repo("https://github.com/rust-lang/rust/");
        assert_eq!(result, Some(("rust-lang".to_string(), "rust".to_string())));
    }

    #[test]
    fn parse_repo_rejects_invalid_input() {
        assert_eq!(parse_github_repo("not-a-repo"), None);
        assert_eq!(parse_github_repo(""), None);
    }

    #[test]
    fn parse_repo_rejects_single_component() {
        assert_eq!(parse_github_repo("rust-lang"), None);
    }
}
