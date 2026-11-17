#[derive(Clone, Copy, Debug)]
pub struct RowStyle {
    pub fg: Option<(u16, u16, u16)>,
    pub bg: Option<(u16, u16, u16)>,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub enum ContextSpec {
    #[default]
    None,
    Before(u32),
    After(u32),
    Both(u32),
}

impl ContextSpec {
    pub fn is_absent(&self) -> bool {
        matches!(self, ContextSpec::None)
    }

    pub fn before(&self) -> u32 {
        match self {
            ContextSpec::Before(n) | ContextSpec::Both(n) => *n,
            _ => 0,
        }
    }

    pub fn after(&self) -> u32 {
        match self {
            ContextSpec::After(n) | ContextSpec::Both(n) => *n,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LineEntry {
    pub line_num: usize,
    pub style: Option<RowStyle>,
    pub match_span: Option<(usize, usize)>,
    pub context_color: Option<(u16, u16, u16)>,
}

#[derive(Debug, Clone)]
pub struct FilterSpec {
    pub filter_type: String,
    pub mode: String,
    pub query: String,
    pub case_insensitive: bool,
    pub style: Option<RowStyle>,
    pub context: ContextSpec,
}

pub fn line_range(offsets: &[usize], line: usize) -> Option<std::ops::Range<usize>> {
    let end = offsets.get(line + 1)?;
    Some(offsets[line]..*end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_range_indexes() {
        let offsets = vec![0, 2, 5, 9]; 
        assert_eq!(line_range(&offsets, 0), Some(0..2));
        assert_eq!(line_range(&offsets, 1), Some(2..5));
        assert_eq!(line_range(&offsets, 2), Some(5..9));
        assert_eq!(line_range(&offsets, 3), None);
        assert_eq!(line_range(&[], 0), None);
    }
}
