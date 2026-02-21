use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
use crate::crates::jobs::common::make_pool;
use futures_util::stream::{FuturesUnordered, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;

type IngestResult<T> = Result<T, anyhow::Error>;

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

struct SessionStateTracker {
    pool: Option<PgPool>,
}

impl SessionStateTracker {
    async fn new(cfg: &Config) -> Self {
        match make_pool(cfg).await {
            Ok(pool) => {
                if let Err(e) = sqlx::query(
                    r#"
                    CREATE TABLE IF NOT EXISTS axon_session_ingest_state (
                        file_path TEXT PRIMARY KEY,
                        last_modified TIMESTAMPTZ NOT NULL,
                        file_size BIGINT NOT NULL,
                        indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                    )
                    "#,
                )
                .execute(&pool)
                .await {
                    log_warn(&format!("failed to ensure session state table: {e}"));
                    return Self { pool: None };
                }
                Self { pool: Some(pool) }
            }
            Err(e) => {
                log_warn(&format!("database connection failed, state tracking disabled: {e}"));
                Self { pool: None }
            }
        }
    }

    async fn should_skip(&self, path: &Path, mtime: SystemTime, size: u64) -> bool {
        let Some(pool) = &self.pool else { return false; };
        let path_str = path.to_string_lossy().to_string();
        
        let row = sqlx::query(
            "SELECT last_modified, file_size FROM axon_session_ingest_state WHERE file_path = $1"
        )
        .bind(path_str)
        .fetch_optional(pool)
        .await;

        match row {
            Ok(Some(r)) => {
                let db_mtime: chrono::DateTime<chrono::Utc> = r.get(0);
                let db_size: i64 = r.get(1);
                let current_mtime: chrono::DateTime<chrono::Utc> = mtime.into();
                
                (db_mtime - current_mtime).num_seconds().abs() < 1 && db_size == (size as i64)
            }
            _ => false,
        }
    }

    async fn mark_indexed(&self, path: &Path, mtime: SystemTime, size: u64) {
        let Some(pool) = &self.pool else { return; };
        let path_str = path.to_string_lossy().to_string();
        let mtime_chrono: chrono::DateTime<chrono::Utc> = mtime.into();

        let _ = sqlx::query(
            r#"
            INSERT INTO axon_session_ingest_state (file_path, last_modified, file_size, indexed_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (file_path) DO UPDATE 
            SET last_modified = EXCLUDED.last_modified, 
                file_size = EXCLUDED.file_size,
                indexed_at = NOW()
            "#
        )
        .bind(path_str)
        .bind(mtime_chrono)
        .bind(size as i64)
        .execute(pool)
        .await;
    }
}

pub async fn ingest_sessions(cfg: &Config) -> Result<usize, Box<dyn Error>> {
    let state = SessionStateTracker::new(cfg).await;
    let multi = MultiProgress::new();
    let main_pb = multi.add(ProgressBar::new_spinner());
    main_pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} [{elapsed_precise}] {msg}")
        .unwrap());
    main_pb.set_message("Discovering session files...");
    main_pb.enable_steady_tick(Duration::from_millis(100));

    let mut total_chunks = 0;
    let all_platforms = !cfg.sessions_claude && !cfg.sessions_codex && !cfg.sessions_gemini;

    if cfg.sessions_claude || all_platforms {
        total_chunks += ingest_claude_sessions(cfg, &state, &multi).await.unwrap_or(0);
    }
    if cfg.sessions_codex || all_platforms {
        total_chunks += ingest_codex_sessions(cfg, &state, &multi).await.unwrap_or(0);
    }
    if cfg.sessions_gemini || all_platforms {
        total_chunks += ingest_gemini_sessions(cfg, &state, &multi).await.unwrap_or(0);
    }

    main_pb.finish_with_message(format!("Ingestion complete: {} chunks embedded", total_chunks));
    Ok(total_chunks)
}

fn resolve_collection(cfg: &Config, derived_name: &str) -> String {
    if cfg.collection != "cortex" {
        return cfg.collection.clone();
    }
    if derived_name.is_empty() {
        return "global-sessions".to_string();
    }
    format!("{}-sessions", derived_name)
}

fn matches_project_filter(cfg: &Config, name: &str) -> bool {
    if let Some(filter) = &cfg.sessions_project {
        name.to_lowercase().contains(&filter.to_lowercase())
    } else {
        true
    }
}

async fn ingest_claude_sessions(cfg: &Config, state: &SessionStateTracker, multi: &MultiProgress) -> IngestResult<usize> {
    let root = expand_home("~/.claude/projects");
    if !root.exists() { return Ok(0); }

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} Claude: {msg}").unwrap());
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut total = 0;
    let mut read_dir = fs::read_dir(root).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut futures = FuturesUnordered::new();

    while let Some(entry) = read_dir.next_entry().await.map_err(|e| anyhow::anyhow!(e.to_string()))? {
        let path = entry.path();
        if path.is_dir() {
            let project_dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
            let clean_name = clean_claude_project_name(project_dir_name);
            
            if !matches_project_filter(cfg, &clean_name) { continue; }

            let collection = resolve_collection(cfg, &clean_name);
            let mut sub_read = fs::read_dir(&path).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
            while let Some(sub_entry) = sub_read.next_entry().await.map_err(|e| anyhow::anyhow!(e.to_string()))? {
                let sub_path = sub_entry.path();
                if sub_path.extension().map_or(false, |ext| ext == "jsonl") {
                    let meta = fs::metadata(&sub_path).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    if state.should_skip(&sub_path, meta.modified().map_err(|e| anyhow::anyhow!(e.to_string()))?, meta.len()).await {
                        continue;
                    }

                    let cfg_clone = cfg.clone();
                    let coll_clone = collection.clone();
                    let mtime = meta.modified().map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    let size = meta.len();
                    
                    futures.push(tokio::spawn(async move {
                        let res = process_claude_file(&cfg_clone, sub_path.clone(), coll_clone).await;
                        (sub_path, mtime, size, res)
                    }));
                    
                    if futures.len() >= 32 {
                        if let Some(res) = futures.next().await {
                            let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                            match r {
                                Ok(count) => { total += count; state.mark_indexed(&p, m, s).await; }
                                Err(e) => log_warn(&format!("Claude file {}: {e}", p.display())),
                            }
                        }
                    }
                }
            }
        }
    }

    while let Some(res) = futures.next().await {
        let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match r {
            Ok(count) => { total += count; state.mark_indexed(&p, m, s).await; }
            Err(e) => log_warn(&format!("Claude file {}: {e}", p.display())),
        }
    }

    pb.finish_with_message(format!("indexed {} chunks", total));
    Ok(total)
}

fn clean_claude_project_name(dir_name: &str) -> String {
    if !dir_name.contains('-') {
        return dir_name.to_string();
    }
    let parts: Vec<&str> = dir_name.trim_start_matches('-').split('-').collect();
    if parts.len() >= 2 {
        let last = parts.last().unwrap();
        let prev = parts[parts.len() - 2];
        if matches!(*last, "rust" | "rs" | "git" | "main" | "master" | "src") {
            format!("{}-{}", prev, last)
        } else {
            last.to_string()
        }
    } else {
        parts.last().unwrap_or(&dir_name).to_string()
    }
}

async fn process_claude_file(cfg: &Config, path: PathBuf, collection: String) -> IngestResult<usize> {
    let content = fs::read_to_string(&path).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut session_text = String::new();
    
    let mut session_cfg = cfg.clone();
    session_cfg.collection = collection;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else { continue };
        
        let role = if val["type"] == "user" { "user" } 
                  else if val["type"] == "assistant" { "assistant" } 
                  else { continue };

        let msg_content = &val["message"]["content"];
        let text = if msg_content.is_string() {
            msg_content.as_str().unwrap().to_string()
        } else if let Some(arr) = msg_content.as_array() {
            let mut combined = String::new();
            for item in arr {
                if let Some(t) = item["text"].as_str() {
                    combined.push_str(t);
                    combined.push('\n');
                }
            }
            combined
        } else {
            continue;
        };

        if !text.trim().is_empty() {
            session_text.push_str(&format!("\n\n### {}:\n{}", role.to_uppercase(), text));
        }
    }

    if session_text.trim().is_empty() { return Ok(0); }

    let url = format!("file://{}", path.display());
    let title = path.file_name().and_then(|n| n.to_str());
    
    let mut attempt = 0;
    loop {
        let res = embed_text_with_metadata(&session_cfg, &session_text, &url, "claude_session", title).await
            .map_err(|e| anyhow::anyhow!(e.to_string()));
        match res {
            Ok(n) => return Ok(n),
            Err(e) => {
                if attempt < 3 {
                    attempt += 1;
                    let wait = Duration::from_millis(attempt * 500);
                    tokio::time::sleep(wait).await;
                    log_warn(&format!("retry {} for {}: {}", attempt, url, e));
                } else {
                    return Err(e);
                }
            }
        }
    }
}

async fn ingest_codex_sessions(cfg: &Config, state: &SessionStateTracker, multi: &MultiProgress) -> IngestResult<usize> {
    let root = expand_home("~/.codex/sessions");
    if !root.exists() { return Ok(0); }

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.yellow} Codex: {msg}").unwrap());
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut total = 0;
    let mut dir_entries = vec![root];
    let mut futures = FuturesUnordered::new();

    while let Some(current_dir) = dir_entries.pop() {
        let mut read_dir = fs::read_dir(current_dir).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| anyhow::anyhow!(e.to_string()))? {
            let path = entry.path();
            if path.is_dir() {
                dir_entries.push(path);
            } else if path.extension().map_or(false, |ext| ext == "jsonl") {
                let meta = fs::metadata(&path).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                if state.should_skip(&path, meta.modified().map_err(|e| anyhow::anyhow!(e.to_string()))?, meta.len()).await { continue; }

                let collection = resolve_collection(cfg, "codex");
                let cfg_clone = cfg.clone();
                let mtime = meta.modified().map_err(|e| anyhow::anyhow!(e.to_string()))?;
                let size = meta.len();
                
                futures.push(tokio::spawn(async move {
                    let res = process_codex_file(&cfg_clone, path.clone(), collection).await;
                    (path, mtime, size, res)
                }));

                if futures.len() >= 32 {
                    if let Some(res) = futures.next().await {
                        let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        match r {
                            Ok(count) => { total += count; state.mark_indexed(&p, m, s).await; }
                            Err(e) => log_warn(&format!("Codex file {}: {e}", p.display())),
                        }
                    }
                }
            }
        }
    }

    while let Some(res) = futures.next().await {
        let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match r {
            Ok(count) => { total += count; state.mark_indexed(&p, m, s).await; }
            Err(e) => log_warn(&format!("Codex file {}: {e}", p.display())),
        }
    }

    pb.finish_with_message(format!("indexed {} chunks", total));
    Ok(total)
}

async fn process_codex_file(cfg: &Config, path: PathBuf, collection: String) -> IngestResult<usize> {
    let content = fs::read_to_string(&path).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut session_text = String::new();
    
    let mut session_cfg = cfg.clone();
    session_cfg.collection = collection;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else { continue };
        if val["type"] != "response_item" { continue; }

        let role = val["payload"]["role"].as_str().unwrap_or("unknown");
        if let Some(arr) = val["payload"]["content"].as_array() {
            let mut combined = String::new();
            for item in arr {
                if let Some(t) = item["text"].as_str() { combined.push_str(t); combined.push('\n'); }
                else if let Some(t) = item["input_text"].as_str() { combined.push_str(t); combined.push('\n'); }
            }
            if !combined.trim().is_empty() {
                session_text.push_str(&format!("\n\n### {}:\n{}", role.to_uppercase(), combined));
            }
        }
    }

    if session_text.trim().is_empty() { return Ok(0); }

    let url = format!("file://{}", path.display());
    let title = path.file_name().and_then(|n| n.to_str());
    
    let mut attempt = 0;
    loop {
        let res = embed_text_with_metadata(&session_cfg, &session_text, &url, "codex_session", title).await
            .map_err(|e| anyhow::anyhow!(e.to_string()));
        match res {
            Ok(n) => return Ok(n),
            Err(e) => {
                if attempt < 3 {
                    attempt += 1;
                    let wait = Duration::from_millis(attempt * 500);
                    tokio::time::sleep(wait).await;
                    log_warn(&format!("retry {} for {}: {}", attempt, url, e));
                } else {
                    return Err(e);
                }
            }
        }
    }
}

async fn ingest_gemini_sessions(cfg: &Config, state: &SessionStateTracker, multi: &MultiProgress) -> IngestResult<usize> {
    let mut total = 0;
    let gemini_root = expand_home("~/.gemini");
    let projects_map = load_gemini_projects(&gemini_root).await;

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.magenta} Gemini: {msg}").unwrap());
    pb.enable_steady_tick(Duration::from_millis(100));

    let paths = [gemini_root.join("history"), gemini_root.join("tmp")];
    let mut futures = FuturesUnordered::new();
    
    for root in paths {
        if !root.exists() { continue; }
        let mut read_dir = fs::read_dir(root).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| anyhow::anyhow!(e.to_string()))? {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                let mut project_name = dir_name.to_string();
                if let Some(mapped) = projects_map.get(dir_name) { project_name = mapped.clone(); }
                else {
                    let root_file = path.join(".project_root");
                    if let Ok(root_path) = fs::read_to_string(root_file).await {
                        if let Some(mapped) = projects_map.get(root_path.trim()) { project_name = mapped.clone(); }
                    }
                }

                if !matches_project_filter(cfg, &project_name) { continue; }

                let collection = resolve_collection(cfg, &project_name);
                let chats_dir = path.join("chats");
                if chats_dir.exists() {
                    let mut chats_read = fs::read_dir(chats_dir).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    while let Some(chat_entry) = chats_read.next_entry().await.map_err(|e| anyhow::anyhow!(e.to_string()))? {
                        let chat_path = chat_entry.path();
                        if chat_path.extension().map_or(false, |ext| ext == "json") {
                            let meta = fs::metadata(&chat_path).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                            if state.should_skip(&chat_path, meta.modified().map_err(|e| anyhow::anyhow!(e.to_string()))?, meta.len()).await { continue; }

                            let cfg_clone = cfg.clone();
                            let coll_clone = collection.clone();
                            let mtime = meta.modified().map_err(|e| anyhow::anyhow!(e.to_string()))?;
                            let size = meta.len();
                            
                            futures.push(tokio::spawn(async move {
                                let res = process_gemini_file(&cfg_clone, chat_path.clone(), coll_clone).await;
                                (chat_path, mtime, size, res)
                            }));

                            if futures.len() >= 32 {
                                if let Some(res) = futures.next().await {
                                    let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                                    match r {
                                        Ok(count) => { total += count; state.mark_indexed(&p, m, s).await; }
                                        Err(e) => log_warn(&format!("Gemini file {}: {e}", p.display())),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    while let Some(res) = futures.next().await {
        let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match r {
            Ok(count) => { total += count; state.mark_indexed(&p, m, s).await; }
            Err(e) => log_warn(&format!("Gemini file {}: {e}", p.display())),
        }
    }

    pb.finish_with_message(format!("indexed {} chunks", total));
    Ok(total)
}

async fn load_gemini_projects(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let projects_file = root.join("projects.json");
    if let Ok(content) = fs::read_to_string(projects_file).await {
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            if let Some(projects) = val["projects"].as_object() {
                for (path, name) in projects {
                    if let Some(n) = name.as_str() {
                        map.insert(path.clone(), n.to_string());
                        if let Some(last) = path.split('/').last() { map.insert(last.to_string(), n.to_string()); }
                    }
                }
            }
        }
    }
    map
}

async fn process_gemini_file(cfg: &Config, path: PathBuf, collection: String) -> IngestResult<usize> {
    let content = fs::read_to_string(&path).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let val: Value = serde_json::from_str(&content).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    
    let mut session_cfg = cfg.clone();
    session_cfg.collection = collection;

    let mut session_text = String::new();
    if let Some(messages) = val["messages"].as_array() {
        for msg in messages {
            let role = msg["type"].as_str().unwrap_or("unknown");
            if let Some(content_arr) = msg["content"].as_array() {
                let mut combined = String::new();
                for item in content_arr {
                    if let Some(t) = item["text"].as_str() { combined.push_str(t); combined.push('\n'); }
                }
                if !combined.trim().is_empty() {
                    session_text.push_str(&format!("\n\n### {}:\n{}", role.to_uppercase(), combined));
                }
            }
        }
    }

    if session_text.trim().is_empty() { return Ok(0); }

    let url = format!("file://{}", path.display());
    let title = path.file_name().and_then(|n| n.to_str());
    
    let mut attempt = 0;
    loop {
        let res = embed_text_with_metadata(&session_cfg, &session_text, &url, "gemini_session", title).await
            .map_err(|e| anyhow::anyhow!(e.to_string()));
        match res {
            Ok(n) => return Ok(n),
            Err(e) => {
                if attempt < 3 {
                    attempt += 1;
                    let wait = Duration::from_millis(attempt * 500);
                    tokio::time::sleep(wait).await;
                    log_warn(&format!("retry {} for {}: {}", attempt, url, e));
                } else {
                    return Err(e);
                }
            }
        }
    }
}
