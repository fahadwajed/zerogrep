
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use memmap2::Mmap;

use super::types::LineEntry;

pub struct SearchSession {
    generation: u64,
    raw: Arc<Mmap>,
    offsets: Arc<Vec<usize>>,
    path: Option<PathBuf>,
    descending: bool,
    results: Vec<LineEntry>,
    line_pos: HashMap<usize, usize>,
    include_matches: Vec<Vec<usize>>,
    row_to_include: Vec<Option<usize>>,
}

impl SearchSession {
    pub fn new(raw: Arc<Mmap>, offsets: Arc<Vec<usize>>) -> Self {
        Self {
            generation: 0,
            raw,
            offsets,
            path: None,
            descending: false,
            results: Vec::new(),
            line_pos: HashMap::new(),
            include_matches: Vec::new(),
            row_to_include: Vec::new(),
        }
    }


    pub fn begin_load(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn begin_search(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_current(&self, gen_id: u64) -> bool {
        gen_id == self.generation
    }


    pub fn finish_load(
        &mut self,
        path: PathBuf,
        raw: Arc<Mmap>,
        offsets: Arc<Vec<usize>>,
        gen_id: u64,
    ) -> bool {
        if !self.is_current(gen_id) {
            return false;
        }
        self.raw = raw;
        self.offsets = offsets;
        self.path = Some(path);
        self.results.clear();
        self.line_pos.clear();
        self.include_matches.clear();
        self.row_to_include.clear();
        true
    }

    pub fn finish_load_failed(&self, gen_id: u64) -> bool {
        self.is_current(gen_id)
    }

    pub fn apply_results(
        &mut self,
        results: Vec<LineEntry>,
        include_matches: Vec<Vec<usize>>,
        row_to_include: Vec<Option<usize>>,
        gen_id: u64,
    ) -> bool {
        if !self.is_current(gen_id) {
            return false;
        }
        self.results = results;
        self.include_matches = include_matches;
        self.row_to_include = row_to_include;
        self.rebuild_line_pos();
        true
    }

    pub fn set_sort_order(&mut self, descending: bool) -> bool {
        if self.descending == descending {
            return false;
        }
        self.descending = descending;
        self.rebuild_line_pos();
        true
    }


    pub fn raw(&self) -> &Arc<Mmap> {
        &self.raw
    }

    pub fn offsets(&self) -> &Arc<Vec<usize>> {
        &self.offsets
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn total_lines(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    pub fn results(&self) -> &[LineEntry] {
        &self.results
    }

    pub fn line_pos(&self) -> &HashMap<usize, usize> {
        &self.line_pos
    }

    pub fn include_matches(&self) -> &[Vec<usize>] {
        &self.include_matches
    }

    pub fn row_to_include(&self) -> &[Option<usize>] {
        &self.row_to_include
    }

    pub fn descending(&self) -> bool {
        self.descending
    }

    fn rebuild_line_pos(&mut self) {
        let n = self.results.len();
        let mut line_pos = HashMap::with_capacity(n.min(100_000));
        for (pos, entry) in self.results.iter().enumerate() {
            line_pos.insert(
                entry.line_num,
                if self.descending { n - 1 - pos } else { pos },
            );
        }
        self.line_pos = line_pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::log_file;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn mapped(content: &[u8]) -> (Arc<Mmap>, Arc<Vec<usize>>) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "zerogrep-session-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, content).unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mapped = unsafe { Mmap::map(&file).unwrap() };
        let offsets = log_file::line_offsets(&mapped);
        let _ = std::fs::remove_file(&path);
        (Arc::new(mapped), Arc::new(offsets))
    }

    fn fresh(content: &[u8]) -> SearchSession {
        let (raw, offsets) = mapped(content);
        SearchSession::new(raw, offsets)
    }

    fn lines_for(entries: &[usize]) -> Vec<LineEntry> {
        entries
            .iter()
            .map(|&line_num| LineEntry {
                line_num,
                style: None,
                match_span: None,
                context_color: None,
            })
            .collect()
    }

    #[test]
    fn stale_search_results_are_dropped() {
        let mut s = fresh(b"a\nb\nc\n");
        let g1 = s.begin_search();
        let g2 = s.begin_search(); 
        assert!(s.apply_results(lines_for(&[0, 2]), vec![], vec![], g2));
        assert!(!s.apply_results(lines_for(&[1]), vec![], vec![], g1));
        let nums: Vec<usize> = s.results().iter().map(|e| e.line_num).collect();
        assert_eq!(nums, vec![0, 2]);
    }

    #[test]
    fn stale_load_is_dropped() {
        let mut s = fresh(b"a\n");
        let g1 = s.begin_load();
        let g2 = s.begin_load(); 
        let (raw2, offsets2) = mapped(b"b\nc\n");
        let p2 = PathBuf::from("/tmp/second.log");
        assert!(s.finish_load(p2.clone(), raw2, offsets2, g2));
        let (raw1, offsets1) = mapped(b"old\n");
        assert!(!s.finish_load(PathBuf::from("/tmp/first.log"), raw1, offsets1, g1));
        assert_eq!(s.path(), Some(p2.as_path()));
        assert_eq!(s.total_lines(), 2);
    }

    #[test]
    fn load_clears_previous_results() {
        let mut s = fresh(b"a\nb\nc\n");
        s.begin_search();
        s.apply_results(
            lines_for(&[0, 2]),
            vec![vec![0usize]],
            vec![Some(0)],
            s.generation(),
        );
        assert_eq!(s.results().len(), 2);
        assert!(!s.line_pos().is_empty());

        let (raw, offsets) = mapped(b"x\n");
        s.begin_load();
        assert!(s.finish_load(PathBuf::from("/tmp/x.log"), raw, offsets, s.generation()));
        assert!(s.results().is_empty());
        assert!(s.line_pos().is_empty());
        assert!(s.include_matches().is_empty());
        assert!(s.row_to_include().is_empty());
    }

    #[test]
    fn stale_load_failure_is_ignored() {
        let mut s = fresh(b"a\n");
        let g1 = s.begin_load();
        s.begin_load();
        assert!(!s.finish_load_failed(g1));
        assert!(s.finish_load_failed(s.generation()));
    }

    #[test]
    fn line_pos_follows_sort_order() {
        let mut s = fresh(b"a\nb\nc\nd\ne\nf\n");
        s.begin_search();
        s.apply_results(lines_for(&[1, 3, 5]), vec![], vec![], s.generation());
        assert_eq!(s.line_pos().get(&1), Some(&0));
        assert_eq!(s.line_pos().get(&5), Some(&2));

        assert!(s.set_sort_order(true));
        assert_eq!(s.line_pos().get(&1), Some(&2));
        assert_eq!(s.line_pos().get(&5), Some(&0));
        assert!(!s.set_sort_order(true));
    }

    #[test]
    fn save_is_invalidated_by_newer_activity() {
        let mut s = fresh(b"a\n");
        let gen_id = s.generation();
        assert!(s.is_current(gen_id));
        s.begin_search();
        assert!(!s.is_current(gen_id));
        s.begin_load();
        assert!(!s.is_current(gen_id));
    }
}
