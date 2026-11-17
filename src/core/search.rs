use super::rules::CompiledFilters;
use super::types::LineEntry;

const DEFAULT_CONTEXT_COLOR: (u16, u16, u16) = (0x8a8a, 0x8686, 0x9494);

#[derive(Debug)]
pub struct SearchOutput {
    pub results: Vec<LineEntry>,
    pub matched_highlights: usize,
    pub include_matches: Vec<Vec<usize>>,
    pub elapsed_ms: u128,
}

pub fn run_search(
    raw: &[u8],
    offsets: &[usize],
    filters: &CompiledFilters,
    highlight_only: bool,
) -> SearchOutput {
    let started = std::time::Instant::now();
    let total_lines = offsets.len().saturating_sub(1);
    let mut matched_highlights = 0usize;
    let mut include_matches: Vec<Vec<usize>> =
        vec![Vec::new(); filters.includes.len()];

    let mut first_match: Vec<Option<usize>> = vec![None; total_lines];
    for line in 0..total_lines {
        let start = offsets[line];
        let end = offsets[line + 1].saturating_sub(1);
        let line_bytes = &raw[start..end];

        if filters
            .excludes
            .iter()
            .any(|rule| rule.matches(line_bytes))
        {
            continue;
        }

        let matching: Vec<usize> = filters
            .includes
            .iter()
            .enumerate()
            .filter(|(_r, (rule, _))| rule.matches(line_bytes))
            .map(|(r, _)| r)
            .collect();
        for r in &matching {
            include_matches[*r].push(line);
        }
        if let Some(&r) = matching.first() {
            first_match[line] = Some(r);
        }
    }

    let mut ctx: Vec<Option<usize>> = vec![None; total_lines];
    for r in 0..filters.includes.len() {
        let spec = filters.include_context[r];
        let before = spec.before() as usize;
        let after = spec.after() as usize;
        if before == 0 && after == 0 {
            continue;
        }
        for &m in &include_matches[r] {
            for line in m.saturating_sub(before)..m {
                if first_match[line].is_none() {
                    ctx[line].get_or_insert(r);
                }
            }
            for line in (m + 1)..(m + 1 + after).min(total_lines) {
                if first_match[line].is_none() {
                    ctx[line].get_or_insert(r);
                }
            }
        }
    }

    let mut results = Vec::with_capacity(total_lines.min(100_000));
    for line in 0..total_lines {
        let start = offsets[line];
        let end = offsets[line + 1].saturating_sub(1);
        let line_bytes = &raw[start..end];

        if filters
            .excludes
            .iter()
            .any(|rule| rule.matches(line_bytes))
        {
            continue;
        }

        if let Some(r) = first_match[line] {
            matched_highlights += 1;
            let style = filters.includes[r].1;
            let match_span = filters
                .includes[r]
                .0
                .match_range(line_bytes)
                .map(|r| (r.start, r.end));
            results.push(LineEntry {
                line_num: line,
                style,
                match_span,
                context_color: None,
            });
        } else if filters.includes.is_empty() || highlight_only {
            results.push(LineEntry {
                line_num: line,
                style: None,
                match_span: None,
                context_color: None,
            });
        } else if let Some(r) = ctx[line] {
            let color = filters
                .includes[r]
                .1
                .and_then(|s| s.fg)
                .unwrap_or(DEFAULT_CONTEXT_COLOR);
            results.push(LineEntry {
                line_num: line,
                style: None,
                match_span: None,
                context_color: Some(color),
            });
        }
    }

    SearchOutput {
        results,
        matched_highlights,
        include_matches,
        elapsed_ms: started.elapsed().as_millis(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rules::compile_filters;
    use crate::core::types::{ContextSpec, FilterSpec};

    fn spec(mode: &str, q: &str) -> FilterSpec {
        FilterSpec {
            filter_type: "Substring".into(),
            mode: mode.into(),
            query: q.into(),
            case_insensitive: false,
            style: None,
            context: ContextSpec::None,
        }
    }

    fn spec_ctx(mode: &str, q: &str, context: ContextSpec) -> FilterSpec {
        FilterSpec {
            filter_type: "Substring".into(),
            mode: mode.into(),
            query: q.into(),
            case_insensitive: false,
            style: Some(crate::core::types::RowStyle {
                fg: Some((0xff00, 0x0000, 0x0000)),
                bg: None,
            }),
            context,
        }
    }

    fn offsets_of(content: &[u8]) -> Vec<usize> {
        crate::core::log_file::line_offsets(content)
    }

    #[test]
    fn no_filters_show_all_lines() {
        let data = b"alpha\nbeta\ngamma\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&[]), false);
        assert_eq!(out.results.len(), 3);
        assert_eq!(out.matched_highlights, 0);
        assert!(out.include_matches.is_empty());
    }

    #[test]
    fn include_keeps_only_matching_lines() {
        let data = b"alpha\nbeta 100\ngamma\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&[spec("Include", "100")]), false);
        assert_eq!(out.results.len(), 1);
        assert_eq!(out.results[0].line_num, 1);
    }

    #[test]
    fn exclude_wins_over_include() {
        let specs = vec![spec("Include", "100"), spec("Exclude", "beta")];
        let data = b"alpha 100\nbeta 100\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        assert_eq!(out.results.len(), 1);
        assert_eq!(out.results[0].line_num, 0);
    }

    #[test]
    fn highlight_only_mode_keeps_nonmatches_unstyled() {
        let data = b"alpha\nbeta 100\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&[spec("Include", "100")]), true);
        assert_eq!(out.results.len(), 2);
        assert_eq!(out.matched_highlights, 1);
        assert!(out.results[0].style.is_none());
    }

    #[test]
    fn include_matches_recorded_per_rule() {
        let specs = vec![spec("Include", "100"), spec("Include", "x")];
        let data = b"a\n100 x\nb\nx\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        assert_eq!(out.include_matches, vec![vec![1usize], vec![1, 3]]);
    }

    #[test]
    fn excluded_lines_are_not_in_include_matches() {
        let specs = vec![spec("Include", "100"), spec("Exclude", "drop")];
        let data = b"100 drop\n100 keep\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        assert_eq!(out.include_matches, vec![vec![1usize]]);
        assert_eq!(out.results.len(), 1);
    }

    #[test]
    fn context_both_shows_lines_around_the_match() {
        let specs = vec![spec_ctx("Include", "100", ContextSpec::Both(1))];
        let data = b"a\nb\n100\nd\ne\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![1, 2, 3]);
        assert!(out.results[0].context_color.is_some());
        assert!(out.results[1].context_color.is_none());
        assert!(out.results[1].match_span.is_some());
        assert!(out.results[2].context_color.is_some());
    }

    #[test]
    fn context_before_and_after_are_separable() {
        let specs = vec![spec_ctx("Include", "100", ContextSpec::Before(1))];
        let data = b"a\nb\n100\nd\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![1, 2]); 

        let specs = vec![spec_ctx("Include", "100", ContextSpec::After(1))];
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![2, 3]); 
    }

    #[test]
    fn context_clamps_at_file_ends() {
        let specs = vec![spec_ctx("Include", "100", ContextSpec::Both(3))];
        let data = b"100\nx\ny\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![0, 1, 2]);
    }

    #[test]
    fn context_lines_are_hidden_in_normal_mode_outside_ranges() {
        let specs = vec![spec("Include", "100")];
        let data = b"a\n100\nb\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![1]);
    }

    #[test]
    fn matching_line_is_never_duplicated_as_context() {
        let specs = vec![spec_ctx("Include", "x", ContextSpec::Both(2))];
        let data = b"a\nx\nb\nx\nd\ne\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(lines.iter().filter(|&&l| l == 2).count(), 1);
    }

    #[test]
    fn excluded_lines_break_nothing_but_stay_hidden_in_context() {
        let specs = vec![
            spec_ctx("Include", "100", ContextSpec::Both(2)),
            spec("Exclude", "drop"),
        ];
        let data = b"a\ndrop\n100\nd\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![0, 2, 3]);
    }

    #[test]
    fn context_of_excluded_match_does_not_leak() {
        let specs = vec![
            spec_ctx("Include", "100", ContextSpec::Both(2)),
            spec("Exclude", "drop"),
        ];
        let data = b"a\n100 drop\nb\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        assert!(out.results.is_empty());
    }

    #[test]
    fn highlight_only_mode_keeps_context_unstyled() {
        let specs = vec![spec_ctx("Include", "100", ContextSpec::Both(1))];
        let data = b"a\nb\n100\nd\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), true);
        let lines: Vec<usize> = out.results.iter().map(|r| r.line_num).collect();
        assert_eq!(lines, vec![0, 1, 2, 3]);
        assert!(out.results.iter().all(|r| r.context_color.is_none()));
    }

    #[test]
    fn context_color_comes_from_the_owning_filter() {
        let specs = vec![spec_ctx("Include", "100", ContextSpec::After(1))];
        let data = b"100\nd\n";
        let out = run_search(data, &offsets_of(data), &compile_filters(&specs), false);
        assert_eq!(out.results[1].context_color, Some((0xff00, 0x0000, 0x0000)));
    }
}
