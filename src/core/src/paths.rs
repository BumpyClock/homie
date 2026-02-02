use std::path::PathBuf;

use directories::BaseDirs;

fn env_home_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        if !profile.is_empty() {
            return Some(PathBuf::from(profile));
        }
    }
    let drive = std::env::var_os("HOMEDRIVE");
    let path = std::env::var_os("HOMEPATH");
    match (drive, path) {
        (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
            Some(PathBuf::from(drive).join(path))
        }
        _ => None,
    }
}

pub fn user_home_dir() -> Option<PathBuf> {
    if let Some(base) = BaseDirs::new() {
        return Some(base.home_dir().to_path_buf());
    }
    env_home_dir()
}

pub fn homie_home_dir() -> Result<PathBuf, String> {
    if let Some(override_dir) = std::env::var_os("HOMIE_HOME") {
        let path = PathBuf::from(override_dir);
        if path.is_relative() {
            return Err("HOMIE_HOME must be an absolute path".to_string());
        }
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("failed to create HOMIE_HOME directory: {e}"))?;
        return Ok(path);
    }

    let home = user_home_dir().ok_or_else(|| {
        "failed to resolve user home; set HOMIE_HOME or HOME/USERPROFILE".to_string()
    })?;
    let dir = home.join(".homie");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create ~/.homie: {e}"))?;
    Ok(dir)
}

pub fn homie_skills_dir() -> Result<PathBuf, String> {
    let dir = homie_home_dir()?;
    let skills_dir = dir.join("skills");
    std::fs::create_dir_all(&skills_dir)
        .map_err(|e| format!("failed to create ~/.homie/skills: {e}"))?;
    Ok(skills_dir)
}
