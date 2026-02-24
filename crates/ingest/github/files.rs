use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
use futures_util::stream::{self, StreamExt};
use reqwest::Client;
use std::error::Error;

use super::{is_indexable_doc_path, is_indexable_source_path};

/// Build a shared reqwest client for GitHub API calls.
pub(super) fn build_client() -> Result<Client, Box<dyn Error>> {
    Ok(Client::builder()
        .user_agent("axon-ingest/1.0 (https://github.com/jmagar/axon_rust)")
        .https_only(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

/// Build a reqwest::RequestBuilder with GitHub auth header applied if a token is available.
pub(super) fn github_request(
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

/// Fetch the repo's recursive file tree and return indexable file paths.
async fn fetch_indexable_files(
    client: &Client,
    owner: &str,
    name: &str,
    default_branch: &str,
    include_source: bool,
    auth_header: Option<&str>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let base = "https://api.github.com";
    let tree_resp: serde_json::Value = github_request(
        client,
        &format!("{base}/repos/{owner}/{name}/git/trees/{default_branch}?recursive=1"),
        auth_header,
    )
    .send()
    .await?
    .error_for_status()?
    .json()
    .await?;

    if tree_resp["truncated"].as_bool().unwrap_or(false) {
        log_warn(&format!(
            "command=ingest_github repo={owner}/{name} tree_truncated=true \
             — large repo, some files skipped"
        ));
    }

    let items = tree_resp["tree"].as_array().cloned().unwrap_or_default();
    Ok(items
        .iter()
        .filter_map(|item| {
            let path = item["path"].as_str()?;
            if item["type"].as_str() != Some("blob") {
                return None;
            }
            let should_index =
                is_indexable_doc_path(path) || (include_source && is_indexable_source_path(path));
            should_index.then(|| path.to_string())
        })
        .collect())
}

/// Fetch and embed all indexable files from the repository concurrently.
pub async fn embed_files(
    cfg: &Config,
    owner: &str,
    name: &str,
    default_branch: &str,
    include_source: bool,
    token: Option<&str>,
) -> Result<usize, Box<dyn Error>> {
    let client = build_client()?;
    let auth: Option<String> = token.map(|t| format!("Bearer {t}"));
    let auth_ref = auth.as_deref();

    let file_items = fetch_indexable_files(
        &client,
        owner,
        name,
        default_branch,
        include_source,
        auth_ref,
    )
    .await?;

    let concurrency = std::cmp::min(cfg.batch_concurrency, 16);
    let results: Vec<Result<usize, String>> = stream::iter(file_items)
        .map(|path| {
            let client = client.clone();
            let cfg = cfg.clone();
            let owner = owner.to_string();
            let name = name.to_string();
            let default_branch = default_branch.to_string();
            let auth_clone = auth.clone();
            async move {
                let raw_url = {
                    let mut url = reqwest::Url::parse("https://raw.githubusercontent.com")
                        .expect("static base URL is valid");
                    url.path_segments_mut()
                        .expect("base URL can be a base")
                        .push(&owner)
                        .push(&name)
                        .push(&default_branch)
                        .extend(path.split('/'));
                    url
                };
                let mut req = client.get(raw_url);
                if let Some(ref a) = auth_clone {
                    req = req.header("Authorization", a.as_str());
                }
                let resp = req.send().await;
                let text = match resp {
                    Ok(r) if r.status().is_success() => match r.text().await {
                        Ok(t) => t,
                        Err(_) => return Ok(0),
                    },
                    _ => return Ok(0),
                };

                if text.trim().is_empty() {
                    return Ok(0);
                }

                let source_url =
                    format!("https://github.com/{owner}/{name}/blob/{default_branch}/{path}");
                match embed_text_with_metadata(&cfg, &text, &source_url, "github", Some(&path))
                    .await
                {
                    Ok(n) => Ok(n),
                    Err(e) => {
                        log_warn(&format!(
                            "command=ingest_github embed_failed path={path} err={e}"
                        ));
                        Ok(0)
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    Ok(results.into_iter().filter_map(|r| r.ok()).sum())
}
