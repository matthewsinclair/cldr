//! The journal's file names, in one place: a date range, then a kebab-case kind.
//!
//! `20261101-20261130-day-notes.md` is a month's file of kind `day-notes`;
//! `20261026-20261101-w44-calendar.csv` is a week's calendar export.

use crate::period::Period;
use jiff::civil::Date;

pub const DAY_NOTES: &str = "day-notes";
pub const CALENDAR: &str = "calendar";

fn day(digits: &str) -> Option<Date> {
  let n = |r: std::ops::Range<usize>| digits.get(r)?.parse::<i16>().ok();
  Date::new(
    n(0..4)?,
    i8::try_from(n(4..6)?).ok()?,
    i8::try_from(n(6..8)?).ok()?,
  )
  .ok()
}

/// `YYYYMMDD-YYYYMMDD` for a span.
pub fn span(first: Date, last: Date) -> String {
  format!("{}-{}", first.strftime("%Y%m%d"), last.strftime("%Y%m%d"))
}

/// A month's file of a kind, eg `20261101-20261130-day-notes.md`.
pub fn month_file(date: Date, kind: &str) -> String {
  let (first, last) = Period::Month(date).days();
  format!("{}-{kind}.md", span(first, last))
}

/// A period's calendar export: a week carries its ISO week number.
pub fn calendar_file(period: Period) -> String {
  let (first, last) = period.days();
  match period {
    Period::Week(_) => format!(
      "{}-w{:02}-{CALENDAR}.csv",
      span(first, last),
      first.iso_week_date().week()
    ),
    Period::Month(_) | Period::Year(_) => format!("{}-{CALENDAR}.csv", span(first, last)),
  }
}

/// A name's date range and the rest after it, eg `day-notes.md`.
pub fn parse(name: &str) -> Option<(Date, Date, &str)> {
  let first = day(name.get(0..8)?)?;
  if name.get(8..9)? != "-" {
    return None;
  }
  let last = day(name.get(9..17)?)?;
  let rest = name.get(17..)?.strip_prefix('-')?;
  (first <= last && !rest.is_empty()).then_some((first, last, rest))
}

/// The month a name belongs to and its kind, when its range is exactly one calendar month and
/// it is a Markdown file.
pub fn parse_month_file(name: &str) -> Option<(Date, &str)> {
  let (first, last, rest) = parse(name)?;
  let kind = rest.strip_suffix(".md")?;
  (first == first.first_of_month() && last == first.last_of_month()).then_some((first, kind))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Result;

  #[test]
  fn month_files_round_trip() -> Result<()> {
    let date = Date::new(2026, 2, 14)?;
    let name = month_file(date, DAY_NOTES);
    assert_eq!(
      parse_month_file(&name),
      Some((date.first_of_month(), DAY_NOTES))
    );
    Ok(())
  }

  #[test]
  fn a_week_export_names_its_week() -> Result<()> {
    let week = Period::Week(Date::new(2026, 10, 28)?);
    let name = calendar_file(week);
    let (first, last, rest) = parse(&name).ok_or("parse")?;
    assert_eq!((first, last), week.days());
    assert_eq!(
      rest,
      format!("w{:02}-{CALENDAR}.csv", first.iso_week_date().week())
    );
    assert!(parse_month_file(&name).is_none());
    Ok(())
  }

  #[test]
  fn other_names_are_not_journal_files() {
    for name in [
      "day_notes.md",
      "20261001-20261031 Day Notes.md",
      "20261001-20261031.pdf",
      "20261031-20261001-day-notes.md",
      "2026 Hours (YTD).md",
    ] {
      assert!(parse_month_file(name).is_none(), "{name}");
    }
  }
}
