
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::types::{ContextSpec, FilterSpec, RowStyle};

pub const KNOWN_TYPES: [&str; 2] = ["Regex", "Substring"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterData {
    pub filter_type: String,
    pub mode: String,
    pub query: String,
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(default, skip_serializing_if = "ContextSpec::is_absent")]
    pub context: ContextSpec,
}

fn default_true() -> bool {
    true
}

impl FilterData {
    pub fn has_known_type(&self) -> bool {
        KNOWN_TYPES.contains(&self.filter_type.as_str())
    }

    pub fn has_known_mode(&self) -> bool {
        self.mode == "Include" || self.mode == "Exclude"
    }

    pub fn style(&self) -> Option<RowStyle> {
        if self.mode == "Exclude" {
            return None;
        }
        let fg = self.fg.as_deref().and_then(color_to_pango);
        let bg = self.bg.as_deref().and_then(color_to_pango);
        if fg.is_none() && bg.is_none() {
            None
        } else {
            Some(RowStyle { fg, bg })
        }
    }

    pub fn to_spec(&self) -> FilterSpec {
        FilterSpec {
            filter_type: self.filter_type.clone(),
            mode: self.mode.clone(),
            query: self.query.clone(),
            case_insensitive: self.case_insensitive,
            style: self.style(),
            context: if self.mode == "Exclude" {
                ContextSpec::None
            } else {
                self.context
            },
        }
    }
}

pub fn color_to_pango(s: &str) -> Option<(u16, u16, u16)> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() < 6 {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        let r = byte(0)?;
        let g = byte(2)?;
        let b = byte(4)?;
        return Some((
            u16::from(r) * 257,
            u16::from(g) * 257,
            u16::from(b) * 257,
        ));
    }

    let body = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<f32> = body
        .split(',')
        .map(|p| p.trim().parse())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() < 3 {
        return None;
    }
    let to16 = |v: f32| (v.clamp(0.0, 255.0) * 257.0) as u16;
    Some((to16(parts[0]), to16(parts[1]), to16(parts[2])))
}

pub fn pango_to_color(c: (u16, u16, u16)) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0 >> 8, c.1 >> 8, c.2 >> 8)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterSet {
    pub name: String,
    pub filters: Vec<FilterData>,
}

pub fn save(path: &Path, set: &FilterSet) -> Result<(), String> {
    let json = serde_json::to_string_pretty(set).map_err(|e| e.to_string())?;
    let mut file =
        File::create(path).map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
    file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.flush()
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    Ok(())
}

pub fn load(path: &Path) -> Result<FilterSet, String> {
    let mut content = String::new();
    let mut file =
        File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    file.read_to_string(&mut content)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Invalid filter file {}: {e}", path.display()))
}

pub fn validate(set: &FilterSet) -> (Vec<FilterData>, Vec<String>) {
    let mut kept = Vec::with_capacity(set.filters.len());
    let mut warnings = Vec::new();

    for (i, f) in set.filters.iter().enumerate() {
        let label = if f.query.is_empty() {
            format!("filter #{}", i + 1)
        } else {
            format!("filter #{} (\"{}\")", i + 1, f.query)
        };

        if !f.has_known_type() {
            warnings.push(format!("{label}: unknown type \"{}\" skipped", f.filter_type));
            continue;
        }
        if f.query.trim().is_empty() {
            warnings.push(format!("{label}: empty query skipped"));
            continue;
        }
        let mut f = f.clone();
        if !f.has_known_mode() {
            warnings.push(format!("{label}: unknown mode \"{}\" treated as Include", f.mode));
            f.mode = "Include".to_string();
        }
        if f.mode == "Exclude" {
            f.context = ContextSpec::None;
        } else {
            f.context = match f.context {
                ContextSpec::Before(0) | ContextSpec::After(0) | ContextSpec::Both(0) => {
                    ContextSpec::None
                }
                other => other,
            };
        }
        kept.push(f);
    }

    (kept, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FilterSet {
        FilterSet {
            name: "test set".into(),
            filters: vec![
                FilterData {
                    filter_type: "Regex".into(),
                    mode: "Include".into(),
                    query: "error|crit".into(),
                    case_insensitive: true,
                    active: true,
                    fg: Some("#ffd959".into()),
                    bg: Some("#731e26".into()),
                    context: ContextSpec::None,
                },
                FilterData {
                    filter_type: "Substring".into(),
                    mode: "Exclude".into(),
                    query: "healthcheck".into(),
                    case_insensitive: true,
                    active: false,
                    fg: None,
                    bg: None,
                    context: ContextSpec::None,
                },
            ],
        }
    }

    #[test]
    fn round_trip_preserves_everything() {
        let set = sample();
        let json = serde_json::to_string_pretty(&set).unwrap();
        let back: FilterSet = serde_json::from_str(&json).unwrap();
        assert_eq!(set, back);
    }

    #[test]
    fn save_and_load_file_round_trip() {
        let set = sample();
        let path = std::env::temp_dir().join(format!(
            "zerogrep-test-{}-{}.zerofilter",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        save(&path, &set).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(set, back);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_fields_default() {
        let json = r#"{
            "name": "old",
            "filters": [{ "filter_type": "Substring", "mode": "Include", "query": "oom" }]
        }"#;
        let set: FilterSet = serde_json::from_str(json).unwrap();
        assert_eq!(set.filters.len(), 1);
        assert!(set.filters[0].active);
        assert!(!set.filters[0].case_insensitive);
        assert!(set.filters[0].fg.is_none());
    }

    #[test]
    fn validate_skips_unknown_and_empty() {
        let set = FilterSet {
            name: "mixed".into(),
            filters: vec![
                FilterData {
                    filter_type: "Semantic".into(),
                    mode: "Include".into(),
                    query: "timeout".into(),
                    case_insensitive: false,
                    active: true,
                    fg: None,
                    bg: None,
                    context: ContextSpec::None,
                },
                FilterData {
                    filter_type: "Regex".into(),
                    mode: "Weird".into(),
                    query: "  ".into(),
                    case_insensitive: false,
                    active: true,
                    fg: None,
                    bg: None,
                    context: ContextSpec::None,
                },
                FilterData {
                    filter_type: "Substring".into(),
                    mode: "Bogus".into(),
                    query: "disk".into(),
                    case_insensitive: false,
                    active: true,
                    fg: None,
                    bg: None,
                    context: ContextSpec::None,
                },
            ],
        };
        let (kept, warnings) = validate(&set);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].query, "disk");
        assert_eq!(kept[0].mode, "Include");
        assert_eq!(warnings.len(), 3);
    }

    #[test]
    fn load_malformed_file_errors() {
        let path = std::env::temp_dir().join(format!(
            "zerogrep-test-{}-bad-{}.zerofilter",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "not json {").unwrap();
        assert!(load(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn stored_colors_survive_round_trip() {
        for (fg, bg) in [
            ("rgb(26,95,180)", "rgb(255,255,255)"),
            ("#1a5fb4", "#ffffff"),
            ("rgba(165,29,45,0.5)", "#f9f06b"),
        ] {
            let d = FilterData {
                filter_type: "Substring".into(),
                mode: "Include".into(),
                query: "x".into(),
                case_insensitive: false,
                active: true,
                fg: Some(fg.into()),
                bg: Some(bg.into()),
                context: ContextSpec::None,
            };
            let style = d.style().expect("Include filter keeps its style");
            assert!(style.fg.is_some(), "fg {fg} should parse");
            assert!(style.bg.is_some(), "bg {bg} should parse");
        }

        let rgb = FilterData {
            filter_type: "Substring".into(),
            mode: "Include".into(),
            query: "x".into(),
            case_insensitive: false,
            active: true,
            fg: Some("rgb(26,95,180)".into()),
            bg: None,
            context: ContextSpec::None,
        }
        .style()
        .unwrap();
        let hex = FilterData {
            filter_type: "Substring".into(),
            mode: "Include".into(),
            query: "x".into(),
            case_insensitive: false,
            active: true,
            fg: Some("#1a5fb4".into()),
            bg: None,
            context: ContextSpec::None,
        }
        .style()
        .unwrap();
        assert_eq!(rgb.fg, hex.fg);

        assert!(color_to_pango("not a color").is_none());
        assert!(color_to_pango("#12").is_none());
        assert_eq!(
            pango_to_color(color_to_pango("#1a5fb4").unwrap()),
            "#1a5fb4"
        );
    }

    #[test]
    fn exclude_filters_are_never_styled() {
        let d = FilterData {
            filter_type: "Substring".into(),
            mode: "Exclude".into(),
            query: "x".into(),
            case_insensitive: false,
            active: true,
            fg: Some("#1a5fb4".into()),
            bg: None,
            context: ContextSpec::None,
        };
        assert!(d.style().is_none());
    }
}
