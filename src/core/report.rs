use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::types::{line_range, LineEntry};

pub fn save_filtered(
    path: &Path,
    raw: &[u8],
    offsets: &[usize],
    entries: &[LineEntry],
    descending: bool,
) -> Result<(), String> {
    let file = File::create(path)
        .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let count = entries.len();
    for pos in 0..count {
        let entry = if descending {
            &entries[count - 1 - pos]
        } else {
            &entries[pos]
        };
        if let Some(range) = line_range(offsets, entry.line_num) {
            writer
                .write_all(&raw[range])
                .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
        }
    }
    writer
        .flush()
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))
}
