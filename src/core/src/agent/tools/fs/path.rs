use std::path::{Path, PathBuf};

pub(super) fn resolve_path(path: &str, cwd: &Path) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = crate::paths::user_home_dir() {
            return Some(home.join(rest));
        }
    }
    if trimmed == "~" {
        return crate::paths::user_home_dir();
    }
    let path_buf = PathBuf::from(trimmed);
    if path_buf.is_relative() {
        return Some(cwd.join(path_buf));
    }
    Some(path_buf)
}
