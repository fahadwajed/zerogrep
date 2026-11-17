use memchr::memmem;
use regex::bytes::Regex as BytesRegex;

use super::types::{ContextSpec, FilterSpec, RowStyle};

#[derive(Debug)]
pub enum CompiledRule {
    Regex(BytesRegex),
    Substring {
        finder: Box<memmem::Finder<'static>>,
        len: usize,
    },
}

impl CompiledRule {
    #[inline(always)]
    pub fn matches(&self, line: &[u8]) -> bool {
        self.match_range(line).is_some()
    }

    #[inline(always)]
    pub fn match_range(&self, line: &[u8]) -> Option<std::ops::Range<usize>> {
        match self {
            CompiledRule::Regex(re) => re.find(line).map(|m| m.start()..m.end()),
            CompiledRule::Substring { finder, len } => {
                finder.find(line).map(|s| s..s + len)
            }
        }
    }
}

pub struct CompiledFilters {
    pub includes: Vec<(CompiledRule, Option<RowStyle>)>,
    pub excludes: Vec<CompiledRule>,
    pub spec_to_include: Vec<Option<usize>>,
    pub include_context: Vec<ContextSpec>,
}

pub fn compile_filters(specs: &[FilterSpec]) -> CompiledFilters {
    let mut includes: Vec<(CompiledRule, Option<RowStyle>)> = Vec::new();
    let mut excludes: Vec<CompiledRule> = Vec::new();
    let mut spec_to_include: Vec<Option<usize>> = Vec::with_capacity(specs.len());
    let mut include_context: Vec<ContextSpec> = Vec::new();

    for cfg in specs {
        let compiled = match cfg.filter_type.as_str() {
            "Regex" => {
                let pattern = if cfg.case_insensitive {
                    format!("(?i){}", cfg.query)
                } else {
                    cfg.query.clone()
                };
                BytesRegex::new(&pattern).ok().map(CompiledRule::Regex)
            }
            "Substring" => {
                if cfg.case_insensitive {
                    let pattern = format!("(?i){}", regex::escape(&cfg.query));
                    BytesRegex::new(&pattern).ok().map(CompiledRule::Regex)
                } else {
                    Some(CompiledRule::Substring {
                        finder: Box::new(memmem::Finder::new(cfg.query.as_bytes()).into_owned()),
                        len: cfg.query.len(),
                    })
                }
            }
            _ => None,
        };

        if let Some(rule) = compiled {
            if cfg.mode == "Include" {
                spec_to_include.push(Some(includes.len()));
                includes.push((rule, cfg.style));
                include_context.push(cfg.context);
            } else {
                spec_to_include.push(None);
                excludes.push(rule);
            }
        } else {
            spec_to_include.push(None);
        }
    }

    CompiledFilters {
        includes,
        excludes,
        spec_to_include,
        include_context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(t: &str, m: &str, q: &str, ci: bool) -> FilterSpec {
        FilterSpec {
            filter_type: t.into(),
            mode: m.into(),
            query: q.into(),
            case_insensitive: ci,
            style: None,
            context: ContextSpec::None,
        }
    }

    #[test]
    fn regex_word_boundary() {
        let f = compile_filters(&[spec("Regex", "Include", r"\b100\b", false)]);
        assert!(f.includes[0].0.matches(b"[x] line 100 - ERROR"));
        assert!(!f.includes[0].0.matches(b"[x] line 1000 - ERROR"));
    }

    #[test]
    fn case_insensitive_regex() {
        let f = compile_filters(&[spec("Regex", "Include", "error", true)]);
        assert!(f.includes[0].0.matches(b"something ERROR happened"));
        assert!(!f.includes[0].0.matches(b"something warn happened"));
    }

    #[test]
    fn case_insensitive_substring_stays_literal() {
        let f = compile_filters(&[spec("Substring", "Include", "a.b", true)]);
        assert!(f.includes[0].0.matches(b"has A.B in it"));
        assert!(!f.includes[0].0.matches(b"has aXb in it"));
    }

    #[test]
    fn sensitive_substring_uses_finder() {
        let f = compile_filters(&[spec("Substring", "Include", "100", false)]);
        match &f.includes[0].0 {
            CompiledRule::Substring { .. } => {}
            other => panic!("expected substring finder, got {:?}", other),
        }
    }

    #[test]
    fn match_range_points_at_the_hit() {
        let f = compile_filters(&[
            spec("Substring", "Include", "100", false),
            spec("Regex", "Include", "ERR\\w+", false),
        ]);
        let line = b"code 100 then ERROR_42";
        assert_eq!(
            f.includes[0].0.match_range(line),
            Some(5..8)
        );
        assert_eq!(
            f.includes[1].0.match_range(line),
            Some(14..22)
        );
        assert_eq!(f.includes[0].0.match_range(b"nothing"), None);
    }

    #[test]
    fn invalid_regex_is_dropped() {
        let f = compile_filters(&[spec("Regex", "Include", "(", false)]);
        assert!(f.includes.is_empty());
    }

    #[test]
    fn modes_route_to_correct_lists() {
        let f = compile_filters(&[
            spec("Substring", "Include", "keep", false),
            spec("Substring", "Exclude", "drop", false),
        ]);
        assert_eq!(f.includes.len(), 1);
        assert_eq!(f.excludes.len(), 1);
        assert!(f.excludes[0].matches(b"pure drop"));
    }
}
