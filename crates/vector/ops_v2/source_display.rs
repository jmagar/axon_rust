use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

struct SourceDisplayResolver {
    manifest_cache: HashMap<PathBuf, HashMap<String, String>>,
    cwd: Option<PathBuf>,
}

impl SourceDisplayResolver {
    fn new() -> Self {
        Self {
            manifest_cache: HashMap::new(),
            cwd: std::env::current_dir().ok(),
        }
    }

    fn display(&mut self, raw: &str) -> String {
        let source = raw.trim();
        if source.is_empty() {
            return raw.to_string();
        }
        if is_web_url(source) {
            return source.to_string();
        }
        if let Some(mapped) = self.map_cached_file_to_url(source) {
            return mapped;
        }
        if looks_like_local_path(source) {
            if let Some(repo_path) = self.repo_display_path(source) {
                return repo_path;
            }
        }
        source.to_string()
    }

    fn map_cached_file_to_url(&mut self, source: &str) -> Option<String> {
        let source_path = PathBuf::from(source);
        let candidates = self.path_candidates(&source_path);
        for candidate in &candidates {
            let Some(manifest_path) = find_manifest_for_path(candidate) else {
                continue;
            };
            let lookup = self.load_manifest_lookup(&manifest_path);
            for key in path_lookup_keys(candidate) {
                if let Some(url) = lookup.get(&key) {
                    return Some(url.clone());
                }
            }
            // Fall back to matching the raw string as stored in payload.
            if let Some(url) = lookup.get(source) {
                return Some(url.clone());
            }
        }
        None
    }

    fn path_candidates(&self, path: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        out.push(path.to_path_buf());
        if let Some(cwd) = &self.cwd {
            if path.is_relative() {
                out.push(cwd.join(path));
            }
        }
        out
    }

    fn load_manifest_lookup(&mut self, manifest_path: &Path) -> &HashMap<String, String> {
        self.manifest_cache
            .entry(manifest_path.to_path_buf())
            .or_insert_with(|| build_manifest_lookup(manifest_path))
    }

    fn repo_display_path(&self, source: &str) -> Option<String> {
        let source_path = PathBuf::from(source);
        for candidate in self.path_candidates(&source_path) {
            let path_for_lookup = if candidate.is_dir() {
                candidate.clone()
            } else {
                candidate
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| candidate.clone())
            };
            let Some(repo_root) = find_repo_root(&path_for_lookup) else {
                continue;
            };
            let rel = if candidate.starts_with(&repo_root) {
                candidate
                    .strip_prefix(&repo_root)
                    .ok()
                    .map(Path::to_path_buf)
            } else {
                None
            };
            let Some(repo_name_os) = repo_root.file_name() else {
                continue;
            };
            let repo_name = repo_name_os.to_string_lossy().to_string();
            let rel_display = rel
                .as_deref()
                .and_then(path_to_forward_string)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| ".".to_string());
            return Some(format!("{repo_name}:{rel_display}"));
        }
        None
    }
}

fn resolver() -> &'static Mutex<SourceDisplayResolver> {
    static RESOLVER: OnceLock<Mutex<SourceDisplayResolver>> = OnceLock::new();
    RESOLVER.get_or_init(|| Mutex::new(SourceDisplayResolver::new()))
}

pub fn display_source(raw: &str) -> String {
    resolver()
        .lock()
        .map(|mut resolver| resolver.display(raw))
        .unwrap_or_else(|_| raw.trim().to_string())
}

pub fn rewrite_diagnostic_source(entry: &str) -> String {
    let Some((prefix, raw_source)) = entry.split_once(" url=") else {
        return entry.to_string();
    };
    format!("{prefix} url={}", display_source(raw_source))
}

fn is_web_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn looks_like_local_path(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let p = Path::new(value);
    p.is_absolute()
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with("~/")
        || value.contains('/')
        || value.contains('\\')
}

fn find_manifest_for_path(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        let candidate = ancestor.join("manifest.jsonl");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn build_manifest_lookup(manifest_path: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(content) = std::fs::read_to_string(manifest_path) else {
        return out;
    };
    let base_dir = manifest_path.parent();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|v| v.as_str()) else {
            continue;
        };
        // Handle both "relative_path" (modern crawl format) and "file_path" (absolute path format).
        let file_path_obj = if let Some(rel) = json
            .get("relative_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let base = base_dir.unwrap_or(Path::new("."));
            base.join(rel)
        } else if let Some(abs) = json
            .get("file_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let p = PathBuf::from(abs);
            out.insert(abs.to_string(), url.to_string());
            p
        } else {
            continue;
        };
        for key in path_lookup_keys(&file_path_obj) {
            out.insert(key, url.to_string());
        }

        if file_path_obj.is_relative() {
            if let Some(base) = base_dir {
                let joined = base.join(&file_path_obj);
                for key in path_lookup_keys(&joined) {
                    out.insert(key, url.to_string());
                }
            }
        }
    }
    out
}

fn path_lookup_keys(path: &Path) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(text) = path_to_forward_string(path) {
        keys.push(text);
    }
    if let Ok(canon) = path.canonicalize() {
        if let Some(text) = path_to_forward_string(&canon) {
            keys.push(text);
        }
    }
    keys
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if ancestor.join(".git").exists() {
            return ancestor
                .canonicalize()
                .ok()
                .or_else(|| Some(ancestor.to_path_buf()));
        }
    }
    None
}

fn path_to_forward_string(path: &Path) -> Option<String> {
    let raw = path.to_str()?;
    Some(raw.replace('\\', "/"))
}
