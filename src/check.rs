//! `cldr check`: the flags, and the comparison with the old Activity Logs, for reading.

use crate::legacy::{self, LoggedDay};
use crate::model::{Data, Flag, Rulebook, Source, round};
use jiff::civil::Date;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Hours closer than this are the same (two decimal places).
const SAME: f64 = 0.005;

#[derive(Debug, Clone, Serialize)]
pub struct Difference {
  pub date: Date,
  pub tag: String,
  pub cldr: f64,
  pub old: f64,
  /// Where cldr took the day from: notes, calendar, or nothing.
  pub source: Option<Source>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Comparison {
  /// Dates the old logs have a day for.
  pub compared: usize,
  pub agree: usize,
  pub cldr_hours: f64,
  pub old_hours: f64,
  /// Per tag over the compared days: (cldr, old).
  pub by_tag: BTreeMap<String, (f64, f64)>,
  pub differences: Vec<Difference>,
  /// Dates with cldr hours that no old log records.
  pub unlogged: Vec<Date>,
}

pub fn compare(data: &Data, logged: &BTreeMap<Date, LoggedDay>, book: &Rulebook<'_>) -> Comparison {
  let mut out = Comparison::default();
  for day in data
    .days
    .iter()
    .filter(|d| d.date >= data.first && d.date <= data.last)
  {
    let Some(log) = logged.get(&day.date) else {
      if day.totals.hours > 0.0 {
        out.unlogged.push(day.date);
      }
      continue;
    };
    out.compared += 1;
    let old = legacy::counted_by_tag(log, book);
    let ours = &day.totals.by_tag;
    let tags: BTreeSet<&String> = ours.keys().chain(old.keys()).collect();
    let mut same = true;
    for tag in tags {
      let a = ours.get(tag).copied().unwrap_or(0.0);
      let b = old.get(tag).copied().unwrap_or(0.0);
      let entry = out.by_tag.entry(tag.clone()).or_default();
      entry.0 += a;
      entry.1 += b;
      out.cldr_hours += a;
      out.old_hours += b;
      if (a - b).abs() > SAME {
        same = false;
        out.differences.push(Difference {
          date: day.date,
          tag: tag.clone(),
          cldr: a,
          old: b,
          source: day.source,
        });
      }
    }
    if same {
      out.agree += 1;
    }
  }
  out.cldr_hours = round(out.cldr_hours);
  out.old_hours = round(out.old_hours);
  for (a, b) in out.by_tag.values_mut() {
    *a = round(*a);
    *b = round(*b);
  }
  out
}

fn source_name(source: Option<Source>) -> &'static str {
  match source {
    Some(Source::Notes) => "notes",
    Some(Source::Calendar) => "calendar",
    None => "nothing",
  }
}

fn flag_line(flag: &Flag) -> String {
  let date = flag.date.map(|d| d.to_string()).unwrap_or_default();
  let at = match (&flag.file, flag.line) {
    (Some(file), Some(line)) => format!(
      "  ({}:{line})",
      file
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default()
    ),
    _ => String::new(),
  };
  format!("  {date:10}  {:16} {}{at}", flag.kind, flag.message)
}

/// The report, as text for a terminal.
pub fn report(data: &Data, comparison: Option<&Comparison>) -> String {
  let mut out = String::new();
  let t = &data.totals;
  let _ = writeln!(
    out,
    "{} to {}: {} h, {} billable",
    data.first, data.last, t.hours, t.billable
  );
  for (tag, hours) in &t.by_tag {
    let billable = t.billable_by_tag.get(tag).copied().unwrap_or(0.0);
    let _ = writeln!(out, "  {tag:16} {hours:8.2} {billable:8.2}");
  }

  let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
  for f in &data.flags {
    *kinds.entry(f.kind).or_default() += 1;
  }
  let _ = writeln!(out, "\nflags: {}", data.flags.len());
  for (kind, n) in &kinds {
    let _ = writeln!(out, "  {kind:16} {n}");
  }
  for flag in &data.flags {
    let _ = writeln!(out, "{}", crate::clean(&flag_line(flag)));
  }

  if let Some(c) = comparison {
    let _ = writeln!(
      out,
      "\nagainst the Activity Logs: {} days compared, {} agree, {} differ; cldr {} h, old {} h",
      c.compared,
      c.agree,
      c.compared - c.agree,
      c.cldr_hours,
      c.old_hours
    );
    for (tag, (a, b)) in &c.by_tag {
      if (a - b).abs() > SAME {
        let _ = writeln!(out, "  {tag:16} cldr {a:8.2}  old {b:8.2}  {:+8.2}", a - b);
      }
    }
    for d in &c.differences {
      let _ = writeln!(
        out,
        "  {}  {:16} cldr {:6.2}  old {:6.2}  {:+6.2}  ({})",
        d.date,
        d.tag,
        d.cldr,
        d.old,
        d.cldr - d.old,
        source_name(d.source)
      );
    }
    if !c.unlogged.is_empty() {
      let _ = writeln!(
        out,
        "  {} days with hours that no old log records",
        c.unlogged.len()
      );
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Rules;
  use crate::model::{Day, Totals};
  use std::path::PathBuf;

  fn day(date: Date, by_tag: &[(&str, f64)]) -> Day {
    let by_tag: BTreeMap<String, f64> = by_tag.iter().map(|(t, h)| ((*t).to_owned(), *h)).collect();
    Day {
      date,
      weekday: "",
      source: Some(Source::Notes),
      code: '0',
      totals: Totals {
        hours: by_tag.values().sum(),
        by_tag,
        ..Totals::default()
      },
      entries: vec![],
    }
  }

  fn logged(date: Date, rows: &[(&str, f64)]) -> LoggedDay {
    LoggedDay {
      date,
      file: PathBuf::from("log.md"),
      line: 1,
      rows: rows
        .iter()
        .map(|(t, h)| legacy::Row {
          tag: (*t).to_owned(),
          hours: *h,
          what: String::new(),
        })
        .collect(),
      total: None,
    }
  }

  #[test]
  fn days_agree_when_every_tag_does() -> crate::Result<()> {
    let first = Date::new(2026, 10, 5)?;
    let second = first.saturating_add(jiff::ToSpan::day(1));
    let third = second.saturating_add(jiff::ToSpan::day(1));
    let data = Data {
      first,
      last: third,
      totals: Totals::default(),
      days: vec![
        day(first, &[("@a", 8.0)]),
        day(second, &[("@a", 8.0)]),
        day(third, &[("@a", 2.0)]),
      ],
      weeks: vec![],
      months: vec![],
      flags: vec![],
    };
    let logs = BTreeMap::from([
      (first, logged(first, &[("@A", 8.0)])),
      (second, logged(second, &[("@a", 7.0), ("@b", 1.0)])),
    ]);
    let (tags, bill, rules) = (BTreeMap::new(), BTreeMap::new(), Rules::default());
    let book = Rulebook::new(&tags, &bill, &rules);
    let c = compare(&data, &logs, &book);
    assert_eq!(c.compared, logs.len());
    assert_eq!(c.agree, 1);
    let tags_differing: Vec<&str> = c.differences.iter().map(|d| d.tag.as_str()).collect();
    assert_eq!(tags_differing, vec!["@a", "@b"]);
    assert_eq!(c.unlogged, vec![third]);
    Ok(())
  }
}
