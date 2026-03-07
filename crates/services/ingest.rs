use crate::crates::core::config::Config;
use crate::crates::ingest;
use crate::crates::services::events::{LogLevel, ServiceEvent, emit};
use crate::crates::services::types::IngestResult;
use std::error::Error;
use tokio::sync::mpsc;

// --- Pure mapping helper (no I/O, testable without live services) ---

pub fn map_ingest_result(payload: serde_json::Value) -> IngestResult {
    IngestResult { payload }
}

// --- Service functions ---

/// Ingest a GitHub repository (code, issues, PRs, wiki) into the vector store.
///
/// Calls `ingest::github::ingest_github` which performs the fetch and embed
/// synchronously. For async/fire-and-forget behaviour use the job queue via
/// the ingest CLI command.
pub async fn ingest_github(
    cfg: &Config,
    owner: &str,
    repo: &str,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<IngestResult, Box<dyn Error>> {
    let repo_slug = format!("{owner}/{repo}");

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("ingesting github repo: {repo_slug}"),
        },
    );

    let chunks = ingest::github::ingest_github(cfg, &repo_slug, cfg.github_include_source).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("github ingest complete: {chunks} chunks"),
        },
    );

    let payload = serde_json::json!({
        "source": "github",
        "repo": repo_slug,
        "chunks": chunks,
    });
    Ok(map_ingest_result(payload))
}

/// Ingest a Reddit subreddit or thread into the vector store.
///
/// `target` may be a subreddit name (e.g. `"rust"`) or a full thread URL.
pub async fn ingest_reddit(
    cfg: &Config,
    target: &str,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<IngestResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("ingesting reddit target: {target}"),
        },
    );

    let chunks = ingest::reddit::ingest_reddit(cfg, target).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("reddit ingest complete: {chunks} chunks"),
        },
    );

    let payload = serde_json::json!({
        "source": "reddit",
        "target": target,
        "chunks": chunks,
    });
    Ok(map_ingest_result(payload))
}

/// Ingest a YouTube video transcript into the vector store.
///
/// `url` may be a full YouTube URL or a bare video ID.
pub async fn ingest_youtube(
    cfg: &Config,
    url: &str,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<IngestResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("ingesting youtube: {url}"),
        },
    );

    let chunks = ingest::youtube::ingest_youtube(cfg, url).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("youtube ingest complete: {chunks} chunks"),
        },
    );

    let payload = serde_json::json!({
        "source": "youtube",
        "url": url,
        "chunks": chunks,
    });
    Ok(map_ingest_result(payload))
}

/// Ingest AI session exports (Claude/Codex/Gemini) into the vector store.
///
/// Session sources and paths are read from cfg (sessions_claude, sessions_codex,
/// sessions_gemini, sessions_project).
pub async fn ingest_sessions(
    cfg: &Config,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<IngestResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "ingesting session exports".to_string(),
        },
    );

    let chunks = ingest::sessions::ingest_sessions(cfg).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("sessions ingest complete: {chunks} chunks"),
        },
    );

    let payload = serde_json::json!({
        "source": "sessions",
        "chunks": chunks,
    });
    Ok(map_ingest_result(payload))
}
