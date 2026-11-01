//! The old Activity Logs, read only, for the transition: the hours the old tooling recorded per
//! day, to compare with what cldr derives. Nothing here writes, and nothing after the transition
//! needs it.
//!
//! What is read: a day heading `#### Weekday YYYY-MM-DD`, then that day's table, whose header
//! names a `Project` and an `Hours` column. A row whose project is `Total` is the day's total.

use crate::Result;
use crate::journal;
use crate::model::Rulebook;
use jiff::civil::Date;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const ACTIVITY_LOG: &str = "activity-log";

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
  pub tag: String,
  pub hours: f64,
  pub what: String,
}

#[derive(Debug, Clone)]
pub struct LoggedDay {
  pub date: Date,
  pub file: PathBuf,
  pub line: usize,
  pub rows: Vec<Row>,
  pub total: Option<f64>,
}

fn cells(line: &str) -> Vec<String> {
  let inner = line.trim().strip_prefix('|').unwrap_or(line.trim());
  let inner = inner.strip_suffix('|').unwrap_or(inner);
  inner.split('|').map(|c| c.trim().to_owned()).collect()
}

/// A day heading: `#### Monday 2026-10-05`.
fn day_heading(line: &str) -> Option<Date> {
  let rest = line.strip_prefix("####")?.trim();
  if rest.starts_with('#') {
    return None;
  }
  let (_, date) = rest.split_once(' ')?;
  date.trim().parse::<Date>().ok()
}

/// Columns found in a table header.
struct Header {
  project: usize,
  hours: usize,
  what: Option<usize>,
}

fn header(cells: &[String]) -> Option<Header> {
  let find = |name: &str| cells.iter().position(|c| c.eq_ignore_ascii_case(name));
  Some(Header {
    project: find("Project")?,
    hours: find("Hours").or_else(|| find("Dur"))?,
    what: find("What"),
  })
}

pub fn parse(text: &str, file: &Path) -> Vec<LoggedDay> {
  let mut days: Vec<LoggedDay> = Vec::new();
  let mut columns: Option<Header> = None;
  let mut in_day = false;
  for (index, line) in text.lines().enumerate() {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
      columns = None;
      in_day = match day_heading(trimmed) {
        Some(date) => {
          days.push(LoggedDay {
            date,
            file: file.to_path_buf(),
            line: index + 1,
            rows: Vec::new(),
            total: None,
          });
          true
        }
        None => false,
      };
      continue;
    }
    if !in_day || !trimmed.starts_with('|') {
      continue;
    }
    let row = cells(trimmed);
    let Some(h) = &columns else {
      columns = header(&row);
      continue;
    };
    let (Some(project), Some(hours)) = (row.get(h.project), row.get(h.hours)) else {
      continue;
    };
    let Ok(hours) = hours.replace(',', ".").parse::<f64>() else {
      continue;
    };
    let Some(day) = days.last_mut() else {
      continue;
    };
    if project.eq_ignore_ascii_case("total") {
      day.total = Some(hours);
    } else if !project.is_empty() {
      day.rows.push(Row {
        tag: crate::clean(project),
        hours,
        what: h
          .what
          .and_then(|i| row.get(i))
          .map(|w| crate::clean(w))
          .unwrap_or_default(),
      });
    }
  }
  days
}

/// Every logged day from `first` to `last`, from every Activity Log under the root. A date in two
/// logs is kept from the log whose name covers it.
pub fn load(root: &Path, first: Date, last: Date) -> Result<BTreeMap<Date, LoggedDay>> {
  let mut out: BTreeMap<Date, LoggedDay> = BTreeMap::new();
  for (path, a, b) in journal::files_of_kind(root, ACTIVITY_LOG)? {
    if b < first || a > last {
      continue;
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    for day in parse(&text, &path) {
      if day.date < first || day.date > last {
        continue;
      }
      let covers = a <= day.date && day.date <= b;
      if covers || !out.contains_key(&day.date) {
        out.insert(day.date, day);
      }
    }
  }
  Ok(out)
}

/// A logged day's counted hours by tag, judged by today's rules.
pub fn counted_by_tag(day: &LoggedDay, book: &Rulebook<'_>) -> BTreeMap<String, f64> {
  let mut by_tag: BTreeMap<String, f64> = BTreeMap::new();
  for row in &day.rows {
    let tag = book.tags.normalise(&row.tag);
    if !book.excluded(&tag, &row.what) {
      *by_tag.entry(tag).or_default() += row.hours;
    }
  }
  by_tag
    .into_iter()
    .map(|(t, h)| (t, crate::model::round(h)))
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Rules;

  const LOG: &str = "## Time\n\n### 2026-10-05 (Week 41)\n\n#### Monday 2026-10-05\n\n\
    | Project | Time        | Hours | What  |\n\
    |---------|-------------|------:|-------|\n\
    | @Acme   | 09:00-11:00 |   2.0 | admin |\n\
    | @Acme   | 11:00-18:30 |   7.5 | build |\n\
    | @Private| 19:00-20:00 |   1.0 | gym   |\n\
    |         |             |       |       |\n\
    | Total   |             |  10.5 |       |\n\n\
    #### Week 41 Summary\n\n\
    | Project  | Mon | Total |\n|---|---:|---:|\n| @Acme | 9.5 | 9.5 |\n";

  #[test]
  fn day_tables_are_read_and_week_summaries_ignored() -> Result<()> {
    let days = parse(LOG, Path::new("log.md"));
    let [day] = days.as_slice() else {
      panic!("one day expected, got {}", days.len());
    };
    assert_eq!(day.date, Date::new(2026, 10, 5)?);
    let sum: f64 = day.rows.iter().map(|r| r.hours).sum();
    assert_eq!(day.total, Some(sum));
    Ok(())
  }

  #[test]
  fn logged_hours_are_judged_by_the_same_rules() -> Result<()> {
    let days = parse(LOG, Path::new("log.md"));
    let tags = BTreeMap::new();
    let bill = BTreeMap::new();
    let rules = Rules {
      exclude: vec!["@private".into()],
      ..Rules::default()
    };
    let book = Rulebook::new(&tags, &bill, &rules);
    let by_tag = counted_by_tag(&days[0], &book);
    let acme: f64 = days[0]
      .rows
      .iter()
      .filter(|r| r.tag == "@Acme")
      .map(|r| r.hours)
      .sum();
    assert_eq!(by_tag, BTreeMap::from([("@acme".to_owned(), acme)]));
    Ok(())
  }
}
