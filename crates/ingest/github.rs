use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
use reqwest::Client;
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

    // Accept known source extensions (MVP scope — covers most common languages;
    // expand as needed for additional language support)
    let accepted = [
        ".rs", ".py", ".go", ".ts", ".js", ".tsx", ".jsx", ".toml", ".c", ".cpp", ".h", ".hpp",
        ".java", ".kt", ".rb", ".php", ".sh", ".yaml", ".yml", ".json", ".md", ".swift", ".cs",
    ];
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

    // Strip .git suffix commonly found in clone URLs
    let repo = repo.strip_suffix(".git").unwrap_or(repo);

    Some((owner.to_string(), repo.to_string()))
}

/// Build a reqwest::RequestBuilder with GitHub auth header applied if a token is available.
fn github_request(
    client: &Client,
    url: &str,
    auth_header: Option<&str>,
) -> reqwest::RequestBuilder {
    let req = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(auth) = auth_header {
        req.header("Authorization", auth)
    } else {
        req
    }
}

/// Ingest a GitHub repository:
/// - Fetches all markdown/doc files unconditionally
/// - Optionally fetches source files when `include_source` is true
/// - Embeds all content into Qdrant via embed_text_with_metadata
pub async fn ingest_github(
    cfg: &Config,
    repo: &str,
    include_source: bool,
) -> Result<usize, Box<dyn Error>> {
    let (owner, name) =
        parse_github_repo(repo).ok_or_else(|| format!("invalid GitHub repo: {repo}"))?;

    let client = Client::builder()
        .user_agent("axon-ingest/1.0 (https://github.com/jmagar/axon_rust)")
        .build()?;

    let auth: Option<String> = cfg.github_token.as_deref().map(|t| format!("Bearer {t}"));
    let auth_ref = auth.as_deref();
    let base = "https://api.github.com";

    // 1. Fetch repo metadata to resolve the default branch
    let repo_info: serde_json::Value =
        github_request(&client, &format!("{base}/repos/{owner}/{name}"), auth_ref)
            .send()
            .await?
            .json()
            .await?;

    if let Some(msg) = repo_info["message"].as_str() {
        return Err(format!("GitHub API error for {owner}/{name}: {msg}").into());
    }

    let default_branch = repo_info["default_branch"]
        .as_str()
        .unwrap_or("main")
        .to_string();

    // 2. Fetch the full recursive file tree
    let tree_resp: serde_json::Value = github_request(
        &client,
        &format!("{base}/repos/{owner}/{name}/git/trees/{default_branch}?recursive=1"),
        auth_ref,
    )
    .send()
    .await?
    .json()
    .await?;

    if tree_resp["truncated"].as_bool().unwrap_or(false) {
        log_warn(&format!(
            "command=ingest_github repo={owner}/{name} tree_truncated=true \
             — large repo, some files skipped"
        ));
    }

    let items = tree_resp["tree"].as_array().cloned().unwrap_or_default();

    let mut count = 0usize;

    for item in &items {
        let path = match item["path"].as_str() {
            Some(p) => p,
            None => continue,
        };
        if item["type"].as_str() != Some("blob") {
            continue;
        }

        let should_index =
            is_indexable_doc_path(path) || (include_source && is_indexable_source_path(path));
        if !should_index {
            continue;
        }

        // Fetch raw file content via raw.githubusercontent.com — avoids base64 decoding
        let raw_url =
            format!("https://raw.githubusercontent.com/{owner}/{name}/{default_branch}/{path}");
        let resp = client.get(&raw_url).send().await;
        let text = match resp {
            Ok(r) if r.status().is_success() => match r.text().await {
                Ok(t) => t,
                Err(_) => continue,
            },
            _ => continue,
        };

        if text.trim().is_empty() {
            continue;
        }

        let source_url = format!("https://github.com/{owner}/{name}/blob/{default_branch}/{path}");
        match embed_text_with_metadata(cfg, &text, &source_url, "github", Some(path)).await {
            Ok(n) => count += n,
            Err(e) => log_warn(&format!(
                "command=ingest_github embed_failed path={path} err={e}"
            )),
        }
    }

    Ok(count)
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

    #[test]
    fn parse_repo_strips_git_suffix() {
        let result = parse_github_repo("https://github.com/rust-lang/rust.git");
        assert_eq!(result, Some(("rust-lang".to_string(), "rust".to_string())));
    }

    #[test]
    fn parse_repo_strips_git_suffix_bare() {
        let result = parse_github_repo("rust-lang/rust.git");
        assert_eq!(result, Some(("rust-lang".to_string(), "rust".to_string())));
    }

    // --- expanded extensions ---

    #[test]
    fn source_path_accepts_c_cpp_files() {
        assert!(is_indexable_source_path("src/main.c"));
        assert!(is_indexable_source_path("src/main.cpp"));
        assert!(is_indexable_source_path("include/header.h"));
        assert!(is_indexable_source_path("include/header.hpp"));
    }

    #[test]
    fn source_path_accepts_java_kotlin_files() {
        assert!(is_indexable_source_path("src/App.java"));
        assert!(is_indexable_source_path("src/App.kt"));
    }

    #[test]
    fn source_path_accepts_ruby_php_shell() {
        assert!(is_indexable_source_path("lib/helper.rb"));
        assert!(is_indexable_source_path("src/index.php"));
        assert!(is_indexable_source_path("scripts/deploy.sh"));
    }

    #[test]
    fn source_path_accepts_yaml_json_md() {
        assert!(is_indexable_source_path("config/settings.yaml"));
        assert!(is_indexable_source_path("config/settings.yml"));
        assert!(is_indexable_source_path("package.json"));
        assert!(is_indexable_source_path("README.md"));
    }
}
