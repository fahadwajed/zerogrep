
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::filter_set::{FilterData, FilterSet};

pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        return std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join("zerogrep"));
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("zerogrep"))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub recent: Vec<PathBuf>,
    #[serde(default)]
    pub filters: Vec<FilterData>,
    #[serde(default)]
    pub descending: bool,
    #[serde(default)]
    pub highlight_only: bool,
}

pub fn session_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("session.json"))
}

pub fn load_session() -> Session {
    session_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_session(session: &Session) {
    if let Some(path) = session_path()
        && let Ok(json) = serde_json::to_string_pretty(session)
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, json);
    }
}

pub fn sets_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("sets"))
}

pub fn list_sets() -> Vec<(String, PathBuf)> {
    let dir = match sets_dir() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut sets: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "zerofilter" {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            Some((name, path))
        })
        .collect();
    sets.sort_by(|a, b| a.0.cmp(&b.0));
    sets
}

fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let collapsed = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    if collapsed.is_empty() {
        "set".to_string()
    } else {
        collapsed
    }
}

pub fn save_set(name: &str, filters: &[FilterData]) -> Result<PathBuf, String> {
    let dir = sets_dir().ok_or("Cannot determine config directory")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.zerofilter", sanitize_name(name)));
    let set = FilterSet {
        name: name.trim().to_string(),
        filters: filters.to_vec(),
    };
    super::filter_set::save(&path, &set)?;
    Ok(path)
}
