use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::paths::homie_skills_dir;

pub(super) fn extract_attached_folder(settings: Option<&Value>) -> Option<String> {
    let settings = settings?;
    let attachments = settings.get("attachments")?;
    if let Some(folder) = attachments.get("folder").and_then(|v| v.as_str()) {
        if !folder.trim().is_empty() {
            return Some(folder.to_string());
        }
    }
    let folders = attachments.get("folders").and_then(|v| v.as_array())?;
    folders
        .iter()
        .filter_map(|v| v.as_str())
        .find(|v| !v.trim().is_empty())
        .map(|v| v.to_string())
}

pub(super) fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".cache"
    )
}

pub(super) fn normalize_search_root(base: &str) -> PathBuf {
    let trimmed = base.trim();
    let home_dir = crate::paths::user_home_dir();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = home_dir.as_ref() {
            return home.join(rest);
        }
    }
    if trimmed == "~" {
        if let Some(home) = home_dir.as_ref() {
            return home.to_path_buf();
        }
    }
    let path = PathBuf::from(trimmed);
    if path.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join(path);
        }
    }
    path
}

pub(super) fn search_files_in_folder(base: &str, query: &str, limit: usize) -> Result<Vec<Value>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let base_path = normalize_search_root(base);
    if !base_path.is_dir() {
        return Ok(Vec::new());
    }

    let mut queue = VecDeque::new();
    queue.push_back(base_path.clone());
    let mut results = Vec::new();
    let mut visited = 0usize;
    let query_lower = query.to_lowercase();

    while let Some(dir) = queue.pop_front() {
        if visited > 25_000 || results.len() >= limit {
            break;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if results.len() >= limit {
                break;
            }
            visited = visited.saturating_add(1);
            if visited > 25_000 {
                break;
            }
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_dir() {
                if should_skip_dir(&name) {
                    continue;
                }
                queue.push_back(path.clone());
            }
            if !file_type.is_file() && !file_type.is_dir() {
                continue;
            }
            let rel = match path.strip_prefix(&base_path) {
                Ok(p) => p,
                Err(_) => Path::new(&name),
            };
            let rel_str = rel.to_string_lossy().to_string();
            let haystack = format!("{name} {rel_str}").to_lowercase();
            if !haystack.contains(&query_lower) {
                continue;
            }
            visited += 1;
            let kind = if file_type.is_dir() {
                "directory"
            } else {
                "file"
            };
            results.push(json!({
                "name": name,
                "path": path.to_string_lossy(),
                "relative_path": rel_str,
                "type": kind,
            }));
        }
    }

    Ok(results)
}

pub(super) fn list_homie_skills() -> Result<Vec<Value>, String> {
    let dir = homie_skills_dir()?;
    let mut skills = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("read skills dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read skills dir entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(value) if !value.trim().is_empty() => value.to_string(),
            _ => continue,
        };
        let path_str = path.to_string_lossy().to_string();
        skills.push(json!({ "name": name, "path": path_str }));
    }
    skills.sort_by(|a, b| {
        let a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a.cmp(b)
    });
    Ok(skills)
}
