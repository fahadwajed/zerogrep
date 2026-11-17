use std::path::Path;
use std::sync::Arc;

use memchr::memchr_iter;
use memmap2::Mmap;

pub fn line_offsets(data: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(1_000_000);
    offsets.push(0);
    for newline_idx in memchr_iter(b'\n', data) {
        offsets.push(newline_idx + 1);
    }
    offsets
}

pub fn open_and_index(path: &Path) -> Option<(Arc<Mmap>, Arc<Vec<usize>>)> {
    let file = std::fs::File::open(path).ok()?;
    let mapped = unsafe { Mmap::map(&file).ok()? };
    let offsets = Arc::new(line_offsets(&mapped));
    Some((Arc::new(mapped), offsets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_for_newline_terminated_data() {
        assert_eq!(line_offsets(b"a\nbb\nccc\n"), vec![0, 2, 5, 9]);
    }

    #[test]
    fn offsets_for_empty_data() {
        assert_eq!(line_offsets(b""), vec![0]);
    }

    #[test]
    fn offsets_ignore_final_unterminated_line() {
        assert_eq!(line_offsets(b"a\nbb"), vec![0, 2]);
    }
}
