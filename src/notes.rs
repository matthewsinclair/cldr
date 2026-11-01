//! The Day Notes: one hand-written Markdown file per month, newest day first.
//!
//! What is read: day headings (`#### YYYYMMDD Weekday`), week headings
//! (`#### YYYYMMDD (Week NN)`), and under each day a `Time Log` heading followed by a table of
//! project tag, time range, hours and what. A row with no tag and some hours is the day's total.
//! Everything else in the file is the user's and is ignored. Reading is lenient: what can be
//! understood is kept, and every departure from the format is reported with its line.

use jiff::civil::{Date, Weekday};
use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};

pub const MINUTES_PER_DAY: u16 = 24 * 60;

/// A parsed Day Notes file.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Notes {
  pub file: PathBuf,
  pub days: Vec<DayNote>,
  pub weeks: Vec<WeekHeading>,
  pub problems: Vec<Problem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeekHeading {
  pub line: usize,
  pub date: Date,
  pub week: i8,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayNote {
  pub date: Date,
  pub line: usize,
  pub rows: Vec<Row>,
  pub total: Option<Total>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Total {
  pub line: usize,
  pub hours: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
  pub line: usize,
  pub tag: String,
  pub span: Option<Span>,
  pub hours: Option<f64>,
  /// The row's billable fraction, when the table has a `Bill` column and the cell is filled.
  pub bill: Option<f64>,
  pub what: String,
}

/// A time range in minutes from the start of its day. `end` is always after `start`; a range
/// that runs past midnight ends after `MINUTES_PER_DAY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
  pub start: u16,
  pub end: u16,
}

impl Span {
  pub fn minutes(self) -> u16 {
    self.end - self.start
  }

  pub fn hours(self) -> f64 {
    f64::from(self.minutes()) / 60.0
  }

  /// Runs past midnight.
  pub fn overnight(self) -> bool {
    self.end > MINUTES_PER_DAY
  }
}

impl fmt::Display for Span {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let clock = |m: u16| format!("{:02}:{:02}", (m / 60) % 24, m % 60);
    write!(f, "{}-{}", clock(self.start), clock(self.end))
  }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
  pub line: usize,
  pub kind: ProblemKind,
  pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemKind {
  /// A heading whose number is not a date.
  BadDate,
  /// The weekday in a heading is not the date's.
  WrongWeekday,
  /// A time range in a form other than `HH:MM-HH:MM`, read anyway.
  OddTime,
  /// A time range that cannot be read.
  BadTime,
  /// Hours in a form other than a decimal with a point, read anyway.
  OddHours,
  /// Hours that cannot be read.
  BadHours,
  /// A billable fraction written as a percentage, read anyway.
  OddBill,
  /// A billable fraction that cannot be read, or is outside 0 to 1.
  BadBill,
  /// More cells in a row than its table has columns; the extra ones are ignored.
  ExtraCells,
  /// Fewer cells in a row than its table has columns; the row is skipped.
  ShortRow,
  /// A time log table before any day heading.
  OutsideDay,
  /// The same date has two headings in one file.
  RepeatedDay,
}

impl fmt::Display for ProblemKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let text = match self {
      ProblemKind::BadDate => "not a date",
      ProblemKind::WrongWeekday => "wrong weekday",
      ProblemKind::OddTime => "time range read leniently",
      ProblemKind::BadTime => "unreadable time range",
      ProblemKind::OddHours => "hours read leniently",
      ProblemKind::BadHours => "unreadable hours",
      ProblemKind::OddBill => "billable fraction read leniently",
      ProblemKind::BadBill => "unreadable billable fraction",
      ProblemKind::ExtraCells => "extra cells",
      ProblemKind::ShortRow => "short row",
      ProblemKind::OutsideDay => "time log outside a day",
      ProblemKind::RepeatedDay => "day repeated",
    };
    f.write_str(text)
  }
}

enum Heading {
  Day(Result<Date, ()>, String),
  Week(Result<Date, ()>, i8),
  TimeLog,
  Other,
}

fn weekday_name(day: Weekday) -> &'static str {
  match day {
    Weekday::Monday => "Monday",
    Weekday::Tuesday => "Tuesday",
    Weekday::Wednesday => "Wednesday",
    Weekday::Thursday => "Thursday",
    Weekday::Friday => "Friday",
    Weekday::Saturday => "Saturday",
    Weekday::Sunday => "Sunday",
  }
}

const WEEKDAYS: [&str; 7] = [
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
  "Sunday",
];

fn date_of(digits: &str) -> Result<Date, ()> {
  let n = |r: std::ops::Range<usize>| digits.get(r).and_then(|s| s.parse::<i16>().ok());
  match (n(0..4), n(4..6), n(6..8)) {
    (Some(y), Some(m), Some(d)) => Date::new(y, m as i8, d as i8).map_err(|_| ()),
    _ => Err(()),
  }
}

/// Classify a heading line (one starting with `#`).
fn heading(line: &str) -> Option<Heading> {
  let level = line.chars().take_while(|c| *c == '#').count();
  if level == 0 {
    return None;
  }
  let rest = line[level..].trim();
  if rest.eq_ignore_ascii_case("time log") {
    return Some(Heading::TimeLog);
  }
  let (digits, tail) = rest.split_at(
    rest
      .find(|c: char| !c.is_ascii_digit())
      .unwrap_or(rest.len()),
  );
  if digits.len() != 8 {
    return Some(Heading::Other);
  }
  let tail = tail.trim();
  if let Some(week) = tail
    .strip_prefix("(Week")
    .and_then(|w| w.trim().strip_suffix(')'))
    .and_then(|w| w.trim().parse::<i8>().ok())
  {
    return Some(Heading::Week(date_of(digits), week));
  }
  let word = tail.split_whitespace().next().unwrap_or("");
  if WEEKDAYS.iter().any(|d| d.eq_ignore_ascii_case(word)) {
    return Some(Heading::Day(date_of(digits), word.to_owned()));
  }
  Some(Heading::Other)
}

/// The cells of a table row, trimmed, without the outer pipes.
fn cells(line: &str) -> Vec<String> {
  let inner = line.trim().strip_prefix('|').unwrap_or(line.trim());
  let inner = inner.strip_suffix('|').unwrap_or(inner);
  inner.split('|').map(|c| c.trim().to_owned()).collect()
}

fn is_separator(cells: &[String]) -> bool {
  cells
    .iter()
    .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// A clock time in minutes; 24:00 is allowed as the end of a day.
fn clock(hour: &str, minute: &str) -> Option<u16> {
  let h: u16 = hour.parse().ok()?;
  let m: u16 = minute.parse().ok()?;
  (h <= 24 && m < 60 && (h < 24 || m == 0)).then_some(h * 60 + m)
}

/// What a table cell held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cell<T> {
  Empty,
  /// In the standard form.
  Plain(T),
  /// Understood, but not in the standard form.
  Lenient(T),
  Unreadable,
}

impl<T> Cell<T> {
  fn is_empty_text(text: &str) -> bool {
    text.is_empty() || text == "-"
  }

  pub fn value(self) -> Option<T> {
    match self {
      Cell::Plain(v) | Cell::Lenient(v) => Some(v),
      Cell::Empty | Cell::Unreadable => None,
    }
  }
}

/// Read a time range, eg `09:00-10:30`.
pub fn parse_span(text: &str) -> Cell<Span> {
  let text = text.trim();
  if Cell::<Span>::is_empty_text(text) {
    return Cell::Empty;
  }
  let groups: Vec<&str> = text
    .split([':', ';', '.', '-', '–'])
    .map(str::trim)
    .collect();
  let [h1, m1, h2, m2] = groups.as_slice() else {
    return Cell::Unreadable;
  };
  let (Some(start), Some(end)) = (clock(h1, m1), clock(h2, m2)) else {
    return Cell::Unreadable;
  };
  if start == MINUTES_PER_DAY {
    return Cell::Unreadable;
  }
  let end = if end <= start {
    end + MINUTES_PER_DAY
  } else {
    end
  };
  let span = Span { start, end };
  let bytes = text.as_bytes();
  let canonical = bytes.len() == "00:00-00:00".len()
    && bytes.get(2) == Some(&b':')
    && bytes.get(5) == Some(&b'-')
    && bytes.get(8) == Some(&b':');
  if canonical {
    Cell::Plain(span)
  } else {
    Cell::Lenient(span)
  }
}

/// Read hours, eg `1.5`.
pub fn parse_hours(text: &str) -> Cell<f64> {
  let text = text.trim();
  if Cell::<f64>::is_empty_text(text) {
    return Cell::Empty;
  }
  match text.replace(',', ".").parse::<f64>() {
    Ok(h) if h.is_finite() && h >= 0.0 && text.contains(',') => Cell::Lenient(h),
    Ok(h) if h.is_finite() && h >= 0.0 => Cell::Plain(h),
    _ => Cell::Unreadable,
  }
}

/// Read a billable fraction, eg `0.5`; `50%` is read leniently.
pub fn parse_bill(text: &str) -> Cell<f64> {
  let text = text.trim();
  if Cell::<f64>::is_empty_text(text) {
    return Cell::Empty;
  }
  let (number, percent) = match text.strip_suffix('%') {
    Some(n) => (n.trim(), true),
    None => (text, false),
  };
  match number.parse::<f64>() {
    Ok(n) if n.is_finite() => {
      let fraction = if percent { n / 100.0 } else { n };
      match (0.0..=1.0).contains(&fraction) {
        true if percent => Cell::Lenient(fraction),
        true => Cell::Plain(fraction),
        false => Cell::Unreadable,
      }
    }
    _ => Cell::Unreadable,
  }
}

/// Where a time log table keeps each value: project, time and hours first, then an optional
/// `Bill`, then what.
#[derive(Debug, Clone, Copy)]
struct Columns {
  width: usize,
  bill: Option<usize>,
}

impl Columns {
  const PLAIN: usize = 4;

  fn from_header(cells: &[String]) -> Columns {
    let bill = cells
      .get(Self::PLAIN - 1)
      .is_some_and(|c| c.eq_ignore_ascii_case("bill"))
      && cells.len() > Self::PLAIN;
    if bill {
      Columns {
        width: Self::PLAIN + 1,
        bill: Some(Self::PLAIN - 1),
      }
    } else {
      Columns {
        width: Self::PLAIN,
        bill: None,
      }
    }
  }

  fn what(self) -> usize {
    self.width - 1
  }
}

enum Log {
  Outside,
  /// After a `Time Log` heading, before its table's header.
  Heading,
  Table(Columns),
}

fn report(problems: &mut Vec<Problem>, line: usize, kind: ProblemKind, text: &str) {
  problems.push(Problem {
    line,
    kind,
    text: crate::clean(text.trim()),
  });
}

/// A cell's value, reporting a lenient or unreadable one.
fn read<T>(
  cell: Cell<T>,
  problems: &mut Vec<Problem>,
  line: usize,
  (lenient, unreadable): (ProblemKind, ProblemKind),
  text: &str,
) -> Option<T> {
  match cell {
    Cell::Lenient(_) => report(problems, line, lenient, text),
    Cell::Unreadable => report(problems, line, unreadable, text),
    Cell::Empty | Cell::Plain(_) => {}
  }
  cell.value()
}

/// Parse one file's text. `file` is recorded for messages only.
pub fn parse(text: &str, file: &Path) -> Notes {
  let mut notes = Notes {
    file: file.to_path_buf(),
    ..Notes::default()
  };
  let mut log = Log::Outside;
  for (index, line) in text.lines().enumerate() {
    let number = index + 1;
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
      log = Log::Outside;
      match heading(trimmed) {
        Some(Heading::Day(Ok(date), word)) => {
          if !word.eq_ignore_ascii_case(weekday_name(date.weekday())) {
            report(
              &mut notes.problems,
              number,
              ProblemKind::WrongWeekday,
              trimmed,
            );
          }
          if notes.days.iter().any(|d| d.date == date) {
            report(
              &mut notes.problems,
              number,
              ProblemKind::RepeatedDay,
              trimmed,
            );
          }
          notes.days.push(DayNote {
            date,
            line: number,
            rows: Vec::new(),
            total: None,
          });
        }
        Some(Heading::Day(Err(()), _)) | Some(Heading::Week(Err(()), _)) => {
          report(&mut notes.problems, number, ProblemKind::BadDate, trimmed);
        }
        Some(Heading::Week(Ok(date), week)) => notes.weeks.push(WeekHeading {
          line: number,
          date,
          week,
        }),
        Some(Heading::TimeLog) => {
          if notes.days.is_empty() {
            report(
              &mut notes.problems,
              number,
              ProblemKind::OutsideDay,
              trimmed,
            );
          } else {
            log = Log::Heading;
          }
        }
        Some(Heading::Other) | None => {}
      }
      continue;
    }
    if matches!(log, Log::Outside) || trimmed.is_empty() {
      continue;
    }
    if !trimmed.starts_with('|') {
      if matches!(log, Log::Table(_)) {
        log = Log::Outside;
      }
      continue;
    }
    let mut row = cells(trimmed);
    let columns = match log {
      Log::Table(columns) => columns,
      Log::Heading | Log::Outside => {
        log = Log::Table(Columns::from_header(&row));
        continue;
      }
    };
    if is_separator(&row) {
      continue;
    }
    if row.len() < columns.width {
      report(&mut notes.problems, number, ProblemKind::ShortRow, trimmed);
      continue;
    }
    if row.len() > columns.width {
      report(
        &mut notes.problems,
        number,
        ProblemKind::ExtraCells,
        trimmed,
      );
      row.truncate(columns.width);
    }
    let span = read(
      parse_span(&row[1]),
      &mut notes.problems,
      number,
      (ProblemKind::OddTime, ProblemKind::BadTime),
      &row[1],
    );
    let hours = read(
      parse_hours(&row[2]),
      &mut notes.problems,
      number,
      (ProblemKind::OddHours, ProblemKind::BadHours),
      &row[2],
    );
    let bill = columns.bill.and_then(|i| {
      read(
        parse_bill(&row[i]),
        &mut notes.problems,
        number,
        (ProblemKind::OddBill, ProblemKind::BadBill),
        &row[i],
      )
    });
    let Some(day) = notes.days.last_mut() else {
      continue;
    };
    let tag = crate::clean(&row[0]);
    let what = crate::clean(&row[columns.what()]);
    if tag.is_empty() {
      // The total row; a row with neither tag nor hours is an empty placeholder.
      if let Some(hours) = hours {
        day.total = Some(Total {
          line: number,
          hours,
        });
      }
      continue;
    }
    day.rows.push(Row {
      line: number,
      tag,
      span,
      hours,
      bill,
      what,
    });
  }
  notes
}

/// The date range a Day Notes file's name declares: `YYYYMMDD-YYYYMMDD-day-notes.md`.
pub fn file_range(path: &Path) -> Option<(Date, Date)> {
  let name = path.file_name()?.to_str()?;
  let (first, last, rest) = crate::names::parse(name)?;
  (rest == format!("{}.md", crate::names::DAY_NOTES)).then_some((first, last))
}

#[cfg(test)]
mod tests {
  use super::*;
  use jiff::ToSpan;

  fn heading_for(date: Date) -> String {
    format!(
      "#### {} {} ",
      date.strftime("%Y%m%d"),
      weekday_name(date.weekday())
    )
  }

  const HEADER: &str = "| Project | Time | Hours | What |\n|---|:---:|---:|---|";

  fn day(date: Date, rows: &[&str]) -> String {
    format!(
      "#### ---------------\n{}\n\n#### Time Log\n\n{HEADER}\n{}\n\n##### [X] Notes\n\nSome prose.\n| not | a | time | log |\n",
      heading_for(date),
      rows.join("\n")
    )
  }

  fn parsed(text: &str) -> Notes {
    parse(text, Path::new("test.md"))
  }

  #[test]
  fn days_rows_and_totals_are_read_with_their_lines() -> crate::Result<()> {
    let date = Date::new(2026, 10, 30)?;
    let rows = [
      "| @a | 08:30-10:00 | 1.5 | Prep, etc |",
      "| @b | 10:00-11:00 | 1.0 | Meeting |",
      "|    |             | 2.5 |         |",
    ];
    let text = day(date, &rows);
    let notes = parsed(&text);
    assert!(notes.problems.is_empty(), "{:?}", notes.problems);
    let [d] = notes.days.as_slice() else {
      panic!("one day expected");
    };
    assert_eq!(d.date, date);
    assert_eq!(d.rows.len(), rows.len() - 1);
    let first_line = text.lines().position(|l| l == rows[0]).map(|i| i + 1);
    assert_eq!(Some(d.rows[0].line), first_line);
    assert_eq!(d.rows[0].tag, "@a");
    assert_eq!(d.rows[0].what, "Prep, etc");
    assert_eq!(d.rows[1].span.map(Span::hours), d.rows[1].hours);
    assert_eq!(d.total.as_ref().map(|t| t.hours), Some(2.5));
    Ok(())
  }

  #[test]
  fn prose_tables_outside_a_time_log_are_ignored() -> crate::Result<()> {
    let date = Date::new(2026, 10, 30)?;
    let notes = parsed(&day(date, &["| @a | 09:00-10:00 | 1.0 | x |"]));
    assert_eq!(notes.days[0].rows.len(), 1);
    Ok(())
  }

  #[test]
  fn newest_first_with_week_headings() -> crate::Result<()> {
    let sunday = Date::new(2026, 11, 1)?;
    let monday = sunday.saturating_sub(6.days());
    let week = monday.iso_week_date().week();
    let text = format!(
      "#### {} (Week {week})\n{}{}",
      monday.strftime("%Y%m%d"),
      day(sunday, &["| @a | 12:00-13:00 | 1.0 | x |"]),
      day(monday, &["| @a | 09:00-10:00 | 1.0 | y |"])
    );
    let notes = parsed(&text);
    assert_eq!(notes.weeks.len(), 1);
    assert_eq!(notes.weeks[0].date, monday);
    assert_eq!(notes.weeks[0].week, week);
    let dates: Vec<Date> = notes.days.iter().map(|d| d.date).collect();
    assert_eq!(dates, vec![sunday, monday]);
    Ok(())
  }

  #[test]
  fn ranges_past_midnight_end_on_the_next_day() {
    let Cell::Plain(span) = parse_span("21:30-03:00") else {
      panic!("a plain range expected");
    };
    assert!(span.overnight());
    assert_eq!(span.hours(), 5.5);
    let Cell::Plain(span) = parse_span("19:30-00:00") else {
      panic!("a plain range expected");
    };
    assert_eq!(span.end, MINUTES_PER_DAY);
    assert!(!span.overnight());
    assert_eq!(parse_span("24:00-01:00"), Cell::Unreadable);
    assert_eq!(parse_span(" - "), Cell::Empty);
  }

  #[test]
  fn odd_ranges_and_hours_are_read_and_reported() -> crate::Result<()> {
    let date = Date::new(2026, 1, 28)?;
    let text = day(
      date,
      &[
        "| @a | 20:00:02:00 | 6,0 | colon for a dash, comma for a point |",
        "| @a | 09;00-10:00 | 1.0 | semicolon |",
        "| @a | soon | lots | unreadable |",
        "| @a | 11:00-12:00 | 1.0 | trailing |junk",
      ],
    );
    let notes = parsed(&text);
    let kinds: Vec<ProblemKind> = notes.problems.iter().map(|p| p.kind).collect();
    assert_eq!(
      kinds,
      vec![
        ProblemKind::OddTime,
        ProblemKind::OddHours,
        ProblemKind::OddTime,
        ProblemKind::BadTime,
        ProblemKind::BadHours,
        ProblemKind::ExtraCells,
      ]
    );
    let rows = &notes.days[0].rows;
    assert_eq!(rows[0].span.map(Span::hours), rows[0].hours);
    assert_eq!(rows[2].span, None);
    assert_eq!(rows[2].hours, None);
    assert_eq!(rows[3].what, "trailing");
    Ok(())
  }

  #[test]
  fn bad_headings_are_reported() -> crate::Result<()> {
    let sunday = Date::new(2026, 3, 15)?;
    let wrong = format!(
      "#### {} {}\n",
      sunday.strftime("%Y%m%d"),
      weekday_name(sunday.weekday().previous())
    );
    let text = format!("{wrong}#### 20250931 Tuesday\n");
    let notes = parsed(&text);
    let kinds: Vec<ProblemKind> = notes.problems.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, vec![ProblemKind::WrongWeekday, ProblemKind::BadDate]);
    assert_eq!(notes.days.len(), 1);
    assert_eq!(notes.days[0].date, sunday);
    Ok(())
  }

  #[test]
  fn a_placeholder_row_is_kept_without_hours_and_an_empty_row_dropped() -> crate::Result<()> {
    let date = Date::new(2026, 10, 31)?;
    let notes = parsed(&day(date, &["| @a |  |  | something |", "|  |  |  |  |"]));
    let d = &notes.days[0];
    assert_eq!(d.rows.len(), 1);
    assert_eq!(d.rows[0].hours, None);
    assert!(d.total.is_none());
    Ok(())
  }

  #[test]
  fn a_repeated_day_is_reported() -> crate::Result<()> {
    let date = Date::new(2026, 6, 27)?;
    let text = format!("{}{}", day(date, &[]), day(date, &[]));
    let notes = parsed(&text);
    assert_eq!(notes.problems.len(), 1);
    assert_eq!(notes.problems[0].kind, ProblemKind::RepeatedDay);
    Ok(())
  }

  #[test]
  fn a_file_name_declares_its_range() -> crate::Result<()> {
    let first = Date::new(2026, 10, 1)?;
    let last = first.last_of_month();
    let name = crate::names::month_file(first, crate::names::DAY_NOTES);
    assert_eq!(file_range(Path::new(&name)), Some((first, last)));
    assert_eq!(file_range(Path::new("day_notes.md")), None);
    Ok(())
  }

  #[test]
  fn a_bill_column_is_read_where_the_header_puts_it() -> crate::Result<()> {
    let date = Date::new(2026, 11, 2)?;
    let text = format!(
      "{}\n\n#### Time Log\n\n| Project | Time | Hours | Bill | What |\n|---|---|---|---|---|\n\
       | @a | 09:00-10:00 | 1.0 | 0.5 | travel |\n\
       | @a | 10:00-11:00 | 1.0 |     | default |\n\
       | @a | 11:00-12:00 | 1.0 | 50% | percent |\n\
       | @a | 12:00-13:00 | 1.0 | 2   | too much |\n\
       |    |             | 4.0 |     |  |\n",
      heading_for(date)
    );
    let notes = parsed(&text);
    let rows = &notes.days[0].rows;
    let bills: Vec<Option<f64>> = rows.iter().map(|r| r.bill).collect();
    assert_eq!(bills, vec![Some(0.5), None, Some(0.5), None]);
    assert_eq!(rows[0].what, "travel");
    let kinds: Vec<ProblemKind> = notes.problems.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, vec![ProblemKind::OddBill, ProblemKind::BadBill]);
    assert_eq!(notes.days[0].total.as_ref().map(|t| t.hours), Some(4.0));
    Ok(())
  }
}
