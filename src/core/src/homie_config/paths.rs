use std::path::PathBuf;

use serde::Deserialize;

use crate::paths::{homie_home_dir, user_home_dir};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub credentials_dir: Option<String>,
    pub execpolicy_path: Option<String>,
}

pub(super) fn resolve_path(value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("path override is empty".to_string());
    }
    let home = user_home_dir();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = home {
            return Ok(home.join(rest));
        }
    }
    if trimmed == "~" {
        if let Some(home) = home {
            return Ok(home);
        }
    }
    let path = PathBuf::from(trimmed);
    if path.is_relative() {
        let base = homie_home_dir()?;
        return Ok(base.join(path));
    }
    Ok(path)
}
