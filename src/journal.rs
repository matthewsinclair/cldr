//! Finding and reading the journal's Day Notes files.

use crate::Result;
use crate::notes::{self, Notes};
use jiff::ToSpan;
use jiff::civil::Date;
use std::path::{Path, PathBuf};

/// How deep under the root Day Notes are looked for (eg `YYYY/YYYYMM/` and one level above).
const DEPTH: usize = 3;

/// Every Day Notes file under `root`, by the date range in its name, in path order.
pub fn day_notes_files(root: &Path) -> Result<Vec<(PathBuf, Date, Date)>> {
  let mut found = Vec::new();
  walk(root, DEPTH, &mut found)?;
  found.sort();
  Ok(found)
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<(PathBuf, Date, Date)>) -> Result<()> {
  let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
  for entry in entries {
    let entry = entry?;
    let path = entry.path();
    let kind = entry.file_type()?;
    if kind.is_dir() && depth > 0 {
      walk(&path, depth - 1, found)?;
    } else if kind.is_file()
      && let Some((first, last)) = notes::file_range(&path)
    {
      found.push((path, first, last));
    }
  }
  Ok(())
}

/// The parsed Day Notes whose names overlap `first..=last`, widened by a week either side so
/// that a spanning week's days are found in either month's file.
pub fn load(root: &Path, first: Date, last: Date) -> Result<Vec<Notes>> {
  let from = first.saturating_sub(7.days());
  let to = last.saturating_add(7.days());
  let mut out = Vec::new();
  for (path, a, b) in day_notes_files(root)? {
    if b < from || a > to {
      continue;
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    out.push(notes::parse(&text, &path));
  }
  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn temp_root(name: &str) -> Result<PathBuf> {
    let root = std::env::temp_dir().join(format!("cldr-{name}-{}", std::process::id()));
    if root.exists() {
      std::fs::remove_dir_all(&root)?;
    }
    std::fs::create_dir_all(&root)?;
    Ok(root)
  }

  #[test]
  fn files_are_found_at_the_root_and_in_month_folders() -> Result<()> {
    let root = temp_root("walk")?;
    let names = [
      "20261001-20261031 Day Notes.md",
      "2026/202609/20260901-20260930 Day Notes.md",
      "_Earlier/2025/202512/20251201-20251231 Day Notes.md",
    ];
    for name in names {
      let path = root.join(name);
      if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
      }
      std::fs::write(&path, "")?;
    }
    std::fs::write(root.join("day_notes.md"), "")?;
    std::fs::write(root.join("20261001-20261031 Link Dump.md"), "")?;
    let found = day_notes_files(&root)?;
    assert_eq!(found.len(), names.len());
    let october = Date::new(2026, 10, 15)?;
    let loaded = load(&root, october, october)?;
    let wanted: Vec<PathBuf> = found
      .iter()
      .filter(|(_, a, b)| *a <= october && october <= *b)
      .map(|(p, _, _)| p.clone())
      .collect();
    let got: Vec<PathBuf> = loaded.into_iter().map(|n| n.file).collect();
    assert_eq!(got, wanted);
    std::fs::remove_dir_all(&root)?;
    Ok(())
  }
}
