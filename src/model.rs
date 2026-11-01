//! The model: entries, days, weeks and months, built from the Day Notes and the calendar by the
//! rules in TN0001. Pure: the inputs are already read, and nothing here touches the disk.

use crate::calendar::{Event, When};
use crate::config::Rules;
use crate::notes::{self, DayNote, MINUTES_PER_DAY, Notes, Span};
use crate::period::Period;
use jiff::ToSpan;
use jiff::civil::{Date, Weekday};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Written hours may differ from their time range by this much without a flag (rounding).
pub const HOURS_TOLERANCE: f64 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
  Notes,
  Calendar,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
  pub date: Date,
  pub start: Option<String>,
  pub end: Option<String>,
  pub hours: f64,
  pub tag: String,
  pub what: String,
  pub source: Source,
  /// False for time that is not work (excluded tags and titles, holds, untagged calendar time).
  pub counted: bool,
  /// The billable fraction: the row's `Bill` cell, else the tag's configured fraction, else 0.
  pub bill: f64,
  /// Hours times the fraction, for counted time; otherwise 0.
  pub billable: f64,
  /// Moved from the next date by the midnight rule.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub carried_from: Option<Date>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file: Option<PathBuf>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub line: Option<usize>,
  #[serde(skip)]
  span: Option<Span>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Flag {
  pub kind: &'static str,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub date: Option<Date>,
  pub message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file: Option<PathBuf>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub line: Option<usize>,
}

/// Hours and billable hours, in all and per tag. Only counted time is in them.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Totals {
  pub hours: f64,
  pub by_tag: BTreeMap<String, f64>,
  pub billable: f64,
  pub billable_by_tag: BTreeMap<String, f64>,
}

impl Totals {
  fn add(&mut self, tag: &str, hours: f64, billable: f64) {
    *self.by_tag.entry(tag.to_owned()).or_default() += hours;
    *self.billable_by_tag.entry(tag.to_owned()).or_default() += billable;
  }

  /// Rounded, with the sums taken from the rounded tags so they always add up.
  fn finish(self) -> Totals {
    let by_tag: BTreeMap<String, f64> = self
      .by_tag
      .into_iter()
      .map(|(t, h)| (t, round(h)))
      .collect();
    let billable_by_tag: BTreeMap<String, f64> = self
      .billable_by_tag
      .into_iter()
      .filter(|(_, h)| *h != 0.0)
      .map(|(t, h)| (t, round(h)))
      .collect();
    Totals {
      hours: round(by_tag.values().sum()),
      billable: round(billable_by_tag.values().sum()),
      by_tag,
      billable_by_tag,
    }
  }

  fn of_entries(entries: &[Entry]) -> Totals {
    let mut t = Totals::default();
    for e in entries.iter().filter(|e| e.counted) {
      t.add(&e.tag, e.hours, e.billable);
    }
    t.finish()
  }

  fn of_days(days: &[&Day]) -> Totals {
    let mut t = Totals::default();
    for d in days {
      for (tag, hours) in &d.totals.by_tag {
        let billable = d.totals.billable_by_tag.get(tag).copied().unwrap_or(0.0);
        t.add(tag, *hours, billable);
      }
    }
    t.finish()
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct Day {
  pub date: Date,
  pub weekday: &'static str,
  pub source: Option<Source>,
  pub code: char,
  #[serde(flatten)]
  pub totals: Totals,
  pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Week {
  pub week: String,
  pub first: Date,
  pub last: Date,
  #[serde(flatten)]
  pub totals: Totals,
}

#[derive(Debug, Clone, Serialize)]
pub struct Month {
  pub month: String,
  pub first: Date,
  pub last: Date,
  #[serde(flatten)]
  pub totals: Totals,
  /// The weeks summarised in this month: those that start in it.
  pub weeks: Vec<String>,
  /// One grid code per day of the month.
  pub grid: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Data {
  pub first: Date,
  pub last: Date,
  #[serde(flatten)]
  pub totals: Totals,
  pub days: Vec<Day>,
  pub weeks: Vec<Week>,
  pub months: Vec<Month>,
  pub flags: Vec<Flag>,
}

/// What the model is built from.
pub struct Inputs<'a> {
  pub period: Period,
  pub notes: &'a [Notes],
  /// None when the calendar was not read.
  pub calendar: Option<&'a [Event]>,
  pub tags: &'a BTreeMap<String, String>,
  /// Billable fraction per tag.
  pub bill: &'a BTreeMap<String, f64>,
  pub rules: &'a Rules,
  /// Days after this take nothing from the calendar and have no flags for missing time.
  pub today: Date,
}

/// Grid code for a day's hours: whole hours, floored; `0` to `9`, then `A` for 10 to `K` for 20.
pub fn grid_code(hours: f64) -> char {
  const LETTERS: &str = "ABCDEFGHIJK";
  let whole = hours.max(0.0).floor();
  if whole < 10.0 {
    char::from_digit(whole as u32, 10).unwrap_or('0')
  } else {
    LETTERS.chars().nth((whole - 10.0) as usize).unwrap_or('+')
  }
}

/// Two decimal places, so sums of hours print as written.
pub fn round(hours: f64) -> f64 {
  // Adding zero turns an empty sum's negative zero into zero.
  (hours * 100.0).round() / 100.0 + 0.0
}

pub fn weekday_name(day: Weekday) -> &'static str {
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

fn weekend(date: Date) -> bool {
  matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday)
}

fn iso_week(date: Date) -> String {
  let iso = date.iso_week_date();
  format!("{}-W{:02}", iso.year(), iso.week())
}

fn clock(minutes: u16) -> String {
  format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}

/// Tag spelling: the configured spelling for any case of a configured key, else lower case.
pub struct Tags<'a> {
  configured: BTreeMap<String, &'a str>,
}

impl<'a> Tags<'a> {
  pub fn new(config: &'a BTreeMap<String, String>) -> Tags<'a> {
    let mut configured = BTreeMap::new();
    for (from, to) in config {
      configured.insert(from.to_lowercase(), to.as_str());
      configured.insert(to.to_lowercase(), to.as_str());
    }
    Tags { configured }
  }

  /// The tag's one spelling. A tag written without `@` gains it.
  pub fn normalise(&self, raw: &str) -> String {
    let raw = raw.trim();
    let tagged = if raw.starts_with('@') {
      raw.to_owned()
    } else {
      format!("@{raw}")
    };
    let key = tagged.to_lowercase();
    match self.configured.get(&key) {
      Some(to) => (*to).to_owned(),
      None => key,
    }
  }
}

/// The tag a calendar title carries: a leading `[Name]` is `@name`.
pub fn calendar_tag(title: &str) -> Option<&str> {
  let rest = title.trim_start().strip_prefix('[')?;
  let (name, _) = rest.split_once(']')?;
  let name = name.trim();
  (!name.is_empty() && !name.contains(char::is_whitespace)).then_some(name)
}

fn starts_with_any(text: &str, prefixes: &[String]) -> bool {
  let text = text.trim_start().to_lowercase();
  prefixes
    .iter()
    .any(|p| !p.is_empty() && text.starts_with(&p.to_lowercase()))
}

/// The configured rules, ready to apply: tag spelling, what counts as work, what bills. The
/// model and the comparison with the old Activity Logs both judge time by this.
pub struct Rulebook<'a> {
  pub tags: Tags<'a>,
  rules: &'a Rules,
  excluded_tags: BTreeSet<String>,
  /// Billable fraction by normalised tag.
  bill: BTreeMap<String, f64>,
}

impl<'a> Rulebook<'a> {
  pub fn new(
    tags: &'a BTreeMap<String, String>,
    bill: &BTreeMap<String, f64>,
    rules: &'a Rules,
  ) -> Rulebook<'a> {
    let tags = Tags::new(tags);
    let excluded_tags = rules
      .exclude
      .iter()
      .filter(|e| e.starts_with('@'))
      .map(|e| tags.normalise(e).to_lowercase())
      .collect();
    let bill = bill
      .iter()
      .map(|(tag, fraction)| (tags.normalise(tag), *fraction))
      .collect();
    Rulebook {
      tags,
      rules,
      excluded_tags,
      bill,
    }
  }

  pub fn bill_for(&self, tag: &str) -> f64 {
    self.bill.get(tag).copied().unwrap_or(0.0)
  }

  /// Not work: an excluded tag, or a title that starts with an excluded prefix.
  pub fn excluded(&self, tag: &str, what: &str) -> bool {
    self.excluded_tags.contains(&tag.to_lowercase()) || starts_with_any(what, &self.rules.exclude)
  }
}

struct Builder<'a> {
  inputs: &'a Inputs<'a>,
  book: Rulebook<'a>,
  flags: Vec<Flag>,
  spellings: BTreeMap<String, BTreeSet<String>>,
}

impl<'a> Builder<'a> {
  fn flag(&mut self, kind: &'static str, date: Option<Date>, message: String) {
    self.flags.push(Flag {
      kind,
      date,
      message,
      file: None,
      line: None,
    });
  }

  fn flag_at(
    &mut self,
    kind: &'static str,
    date: Option<Date>,
    message: String,
    file: &std::path::Path,
    line: usize,
  ) {
    self.flags.push(Flag {
      kind,
      date,
      message,
      file: Some(file.to_path_buf()),
      line: Some(line),
    });
  }

  /// A Day Notes tag, its spelling recorded for the consistency flag.
  fn tag(&mut self, raw: &str) -> String {
    let tag = self.book.tags.normalise(raw);
    self
      .spellings
      .entry(tag.clone())
      .or_default()
      .insert(raw.trim().to_owned());
    tag
  }

  /// The Day Notes for a date: from the file whose name covers it, else the first that has it.
  /// Another file's different copy of the same day is flagged.
  fn note_for(&mut self, date: Date) -> Option<(&'a Notes, &'a DayNote)> {
    let mut found: Vec<(&'a Notes, &'a DayNote)> = Vec::new();
    for file in self.inputs.notes {
      if let Some(day) = file.days.iter().find(|d| d.date == date) {
        found.push((file, day));
      }
    }
    let covers =
      |n: &Notes| notes::file_range(&n.file).is_some_and(|(a, b)| a <= date && date <= b);
    let chosen = found
      .iter()
      .position(|(n, _)| covers(n))
      .or(if found.is_empty() { None } else { Some(0) })?;
    let (file, day) = found[chosen];
    for (other, copy) in found
      .iter()
      .enumerate()
      .filter(|(i, _)| *i != chosen)
      .map(|(_, f)| f)
    {
      if copy.rows != day.rows {
        self.flag_at(
          "day_differs",
          Some(date),
          format!("a different copy is in {}", other.file.display()),
          &file.file,
          day.line,
        );
      }
    }
    Some((file, day))
  }

  fn notes_entries(&mut self, file: &Notes, day: &DayNote) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut sum = 0.0;
    for row in &day.rows {
      let tag = self.tag(&row.tag);
      let span_hours = row.span.map(Span::hours);
      let hours = match (row.hours, span_hours) {
        (Some(h), Some(s)) => {
          if (h - s).abs() > HOURS_TOLERANCE {
            self.flag_at(
              "hours_disagree",
              Some(day.date),
              format!(
                "{h} h written for {}",
                row.span.map(|s| s.to_string()).unwrap_or_default()
              ),
              &file.file,
              row.line,
            );
          }
          h
        }
        (Some(h), None) => h,
        (None, Some(s)) => {
          self.flag_at(
            "no_hours",
            Some(day.date),
            format!("{tag} has a time range but no hours; the range is used"),
            &file.file,
            row.line,
          );
          s
        }
        (None, None) => {
          self.flag_at(
            "no_hours",
            Some(day.date),
            format!("{tag} has neither time nor hours"),
            &file.file,
            row.line,
          );
          0.0
        }
      };
      sum += row.hours.unwrap_or(0.0);
      let counted = !self.book.excluded(&tag, &row.what);
      let bill = row.bill.unwrap_or_else(|| self.book.bill_for(&tag));
      entries.push(Entry {
        date: day.date,
        start: row.span.map(|s| clock(s.start)),
        end: row.span.map(|s| clock(s.end)),
        hours,
        tag,
        what: row.what.clone(),
        source: Source::Notes,
        counted,
        bill,
        billable: if counted { hours * bill } else { 0.0 },
        carried_from: None,
        file: Some(file.file.clone()),
        line: Some(row.line),
        span: row.span,
      });
    }
    self.flag_overlaps(file, day);
    if let Some(total) = &day.total
      && round(total.hours) != round(sum)
    {
      self.flag_at(
        "total_disagrees",
        Some(day.date),
        format!(
          "total row says {} but the rows add to {}",
          total.hours,
          round(sum)
        ),
        &file.file,
        total.line,
      );
    }
    entries
  }

  /// Day Notes rows whose time ranges overlap count the shared time twice; each pair is flagged.
  fn flag_overlaps(&mut self, file: &Notes, day: &DayNote) {
    let mut timed: Vec<(&notes::Row, Span)> = day
      .rows
      .iter()
      .filter_map(|r| r.span.map(|s| (r, s)))
      .collect();
    timed.sort_by_key(|(_, s)| (s.start, s.end));
    for (i, (a, sa)) in timed.iter().enumerate() {
      for (b, sb) in timed.iter().skip(i + 1) {
        if sb.start >= sa.end {
          break;
        }
        let shared = sa.end.min(sb.end) - sb.start;
        self.flag_at(
          "notes_overlap",
          Some(day.date),
          format!(
            "{sa} and {sb} overlap: {} h counted twice",
            round(f64::from(shared) / 60.0)
          ),
          &file.file,
          b.line.max(a.line),
        );
      }
    }
  }

  /// Timed calendar events on a date, each clipped to time not already covered (net time), and
  /// the minutes of overlap removed.
  fn calendar_entries(&mut self, date: Date, events: &[Event]) -> (Vec<Entry>, u16) {
    let mut timed: Vec<(Span, &Event)> = events
      .iter()
      .filter_map(|e| match &e.when {
        When::Timed { start, end } if start.date() == date => {
          let from = u16::try_from(start.hour()).ok()? * 60 + u16::try_from(start.minute()).ok()?;
          let minutes = u16::try_from(end.duration_since(start).as_mins()).ok()?;
          (minutes > 0).then_some((
            Span {
              start: from,
              end: from.saturating_add(minutes),
            },
            e,
          ))
        }
        _ => None,
      })
      .collect();
    timed.sort_by_key(|(s, _)| (s.start, std::cmp::Reverse(s.end)));
    let mut covered_to = 0u16;
    let mut overlap = 0u16;
    let mut entries = Vec::new();
    for (span, event) in timed {
      let title = crate::export::one_line(&event.title);
      let tag = calendar_tag(&title).map(|t| self.book.tags.normalise(t));
      let hold = starts_with_any(&title, &self.inputs.rules.zero_hours);
      let counted = match &tag {
        Some(tag) => !hold && !self.book.excluded(tag, &title),
        None => false,
      };
      let net = if counted {
        let from = span.start.max(covered_to);
        let net = span.end.saturating_sub(from);
        overlap += span.minutes() - net;
        covered_to = covered_to.max(span.end);
        net
      } else {
        0
      };
      let hours = if counted {
        f64::from(net) / 60.0
      } else {
        f64::from(span.minutes()) / 60.0
      };
      let tag = tag.unwrap_or_default();
      let bill = self.book.bill_for(&tag);
      entries.push(Entry {
        date,
        start: Some(clock(span.start)),
        end: Some(clock(span.end)),
        hours,
        tag,
        what: title,
        source: Source::Calendar,
        counted,
        bill,
        billable: if counted { hours * bill } else { 0.0 },
        carried_from: None,
        file: None,
        line: None,
        span: Some(span),
      });
    }
    (entries, overlap)
  }
}

/// The midnight rule: entries on `next` that start where a session on `day` ended, past midnight,
/// belong to `day`. Chains of contiguous entries move together.
fn carry_midnight(day: &mut Vec<Entry>, next: &mut Vec<Entry>, next_date: Date, day_date: Date) {
  loop {
    let ends: BTreeSet<u16> = day
      .iter()
      .filter_map(|e| e.span.map(|s| s.end))
      .filter(|end| *end >= MINUTES_PER_DAY)
      .collect();
    let Some(index) = next.iter().position(|e| {
      e.source == day.first().map_or(e.source, |d| d.source)
        && e
          .span
          .is_some_and(|s| ends.contains(&(s.start + MINUTES_PER_DAY)))
    }) else {
      return;
    };
    let mut entry = next.remove(index);
    if let Some(s) = entry.span {
      entry.span = Some(Span {
        start: s.start + MINUTES_PER_DAY,
        end: s.end + MINUTES_PER_DAY,
      });
    }
    entry.carried_from = Some(next_date);
    entry.date = day_date;
    day.push(entry);
  }
}

pub fn build(inputs: &Inputs<'_>) -> Data {
  let mut builder = Builder {
    inputs,
    book: Rulebook::new(inputs.tags, inputs.bill, inputs.rules),
    flags: Vec::new(),
    spellings: BTreeMap::new(),
  };

  let (first, last) = inputs.period.days();
  // Whole weeks, so a week at either end is summarised in full; then one day either side, for
  // the midnight rule.
  let (span_first, span_last) = (Period::Week(first).days().0, Period::Week(last).days().1);
  let before = span_first.saturating_sub(1.day());
  let after = span_last.saturating_add(1.day());
  let in_period = |d: Date| d >= first && d <= last;

  for file in inputs.notes {
    for problem in &file.problems {
      let date = file
        .days
        .iter()
        .rev()
        .find(|d| d.line <= problem.line)
        .map(|d| d.date);
      if date.is_none_or(in_period) {
        builder.flag_at(
          "format",
          date,
          format!("{}: {}", problem.kind, problem.text),
          &file.file,
          problem.line,
        );
      }
    }
  }

  let mut per_day: BTreeMap<Date, (Option<Source>, Vec<Entry>)> = BTreeMap::new();
  for date in before.series(1.day()).take_while(|d| *d <= after) {
    let notes = builder.note_for(date);
    let from_notes = notes
      .map(|(file, day)| builder.notes_entries(file, day))
      .unwrap_or_default();
    // Planned time is not worked time: the calendar speaks only for days up to today.
    let from_calendar = inputs
      .calendar
      .filter(|_| date <= inputs.today)
      .map(|events| builder.calendar_entries(date, events));
    let (source, entries) = if !from_notes.is_empty() {
      if let Some((cal, _)) = &from_calendar {
        let notes_hours: f64 = from_notes
          .iter()
          .filter(|e| e.counted)
          .map(|e| e.hours)
          .sum();
        let cal_hours: f64 = cal.iter().filter(|e| e.counted).map(|e| e.hours).sum();
        if (notes_hours - cal_hours).abs() > inputs.rules.max_gap_hours {
          builder.flag(
            "sources_differ",
            Some(date),
            format!(
              "Day Notes {} h, calendar {} h",
              round(notes_hours),
              round(cal_hours)
            ),
          );
        }
      }
      (Some(Source::Notes), from_notes)
    } else {
      match from_calendar {
        Some((cal, overlap)) if cal.iter().any(|e| e.counted) => {
          if overlap > 0 && in_period(date) {
            builder.flag(
              "calendar_overlap",
              Some(date),
              format!(
                "overlapping calendar blocks counted once ({} h overlap)",
                round(f64::from(overlap) / 60.0)
              ),
            );
          }
          (Some(Source::Calendar), cal)
        }
        _ => (None, Vec::new()),
      }
    };
    per_day.insert(date, (source, entries));
  }

  // The midnight rule, newest last so that chains carry back one day at a time.
  let dates: Vec<Date> = per_day.keys().copied().collect();
  for pair in dates.windows(2).rev() {
    let (day, next) = (pair[0], pair[1]);
    let Some((_, mut next_entries)) = per_day.remove(&next) else {
      continue;
    };
    if let Some((_, day_entries)) = per_day.get_mut(&day) {
      carry_midnight(day_entries, &mut next_entries, next, day);
    }
    let source = if next_entries.is_empty() {
      None
    } else {
      next_entries.first().map(|e| e.source)
    };
    per_day.insert(next, (source, next_entries));
  }

  let mut days = Vec::new();
  for (date, (source, entries)) in per_day {
    if date < span_first || date > span_last {
      continue;
    }
    let totals = Totals::of_entries(&entries);
    let hours = totals.hours;
    let past = date <= inputs.today;
    if past && !weekend(date) {
      if source.is_none() {
        builder.flag(
          "empty_day",
          Some(date),
          "a weekday with neither Day Notes nor calendar time".into(),
        );
      } else if hours < inputs.rules.min_weekday_hours {
        builder.flag(
          "short_day",
          Some(date),
          format!("{hours} h, under {} h", inputs.rules.min_weekday_hours),
        );
      }
    }
    if weekend(date) {
      let weekday_only: BTreeSet<String> = inputs
        .rules
        .weekday_only
        .iter()
        .map(|t| builder.book.tags.normalise(t))
        .collect();
      for tag in totals.by_tag.keys().filter(|t| weekday_only.contains(*t)) {
        builder.flag(
          "weekend_tag",
          Some(date),
          format!("{tag} on a {}", weekday_name(date.weekday())),
        );
      }
    }
    if source == Some(Source::Calendar) {
      builder.flag(
        "calendar_only",
        Some(date),
        "no Day Notes; hours from the calendar".into(),
      );
    }
    days.push(Day {
      date,
      weekday: weekday_name(date.weekday()),
      source,
      code: grid_code(hours),
      totals,
      entries,
    });
  }

  for (tag, raw) in std::mem::take(&mut builder.spellings) {
    if raw.len() > 1 {
      let list: Vec<String> = raw.into_iter().collect();
      builder.flag(
        "tag_spellings",
        None,
        format!("{tag} is written {}", list.join(", ")),
      );
    }
  }

  let mut weeks = Vec::new();
  let mut monday = span_first;
  while monday <= span_last {
    let sunday = monday.saturating_add(6.days());
    let in_week: Vec<&Day> = days
      .iter()
      .filter(|d| d.date >= monday && d.date <= sunday)
      .collect();
    weeks.push(Week {
      week: iso_week(monday),
      first: monday,
      last: sunday,
      totals: Totals::of_days(&in_week),
    });
    monday = monday.saturating_add(7.days());
  }

  let mut months = Vec::new();
  let mut start = first.first_of_month();
  while start <= last {
    let end = start.last_of_month();
    let in_month: Vec<&Day> = days
      .iter()
      .filter(|d| d.date >= start && d.date <= end && in_period(d.date))
      .collect();
    if !in_month.is_empty() {
      months.push(Month {
        month: start.strftime("%Y-%m").to_string(),
        first: start,
        last: end,
        totals: Totals::of_days(&in_month),
        weeks: weeks
          .iter()
          .filter(|w| w.first >= start && w.first <= end)
          .map(|w| w.week.clone())
          .collect(),
        grid: in_month.iter().map(|d| d.code).collect(),
      });
    }
    start = start.saturating_add(1.month());
  }

  let all: Vec<&Day> = days.iter().filter(|d| in_period(d.date)).collect();
  let totals = Totals::of_days(&all);
  let mut flags = builder.flags;
  flags.retain(|f| f.date.is_none_or(in_period));
  flags.sort_by(|a, b| (a.date, a.kind).cmp(&(b.date, b.kind)));
  Data {
    first,
    last,
    totals,
    days,
    weeks,
    months,
    flags,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Result;
  use jiff::tz::TimeZone;
  use std::path::Path;

  const HEADER: &str = "| Project | Time | Hours | What |\n|---|:---:|---:|---|";

  /// A Day Notes file for the dates given, newest first, as the user writes it.
  fn file(name: &str, days: &[(Date, &[&str])]) -> Notes {
    let mut text = String::new();
    let mut sorted: Vec<&(Date, &[&str])> = days.iter().collect();
    sorted.sort_by_key(|(d, _)| std::cmp::Reverse(*d));
    for (date, rows) in sorted {
      text.push_str(&format!(
        "#### ---------------\n#### {} {}\n\n#### Time Log\n\n{HEADER}\n{}\n\n",
        date.strftime("%Y%m%d"),
        weekday_name(date.weekday()),
        rows.join("\n")
      ));
    }
    notes::parse(&text, Path::new(name))
  }

  fn month_file(date: Date, days: &[(Date, &[&str])]) -> Notes {
    let name = crate::names::month_file(date, crate::names::DAY_NOTES);
    file(&name, days)
  }

  fn event(title: &str, date: Date, from: (i8, i8), minutes: i64) -> Result<Event> {
    let start = date.at(from.0, from.1, 0, 0).to_zoned(TimeZone::UTC)?;
    let end = start.checked_add(minutes.minutes())?;
    Ok(Event {
      uid: title.into(),
      title: title.into(),
      when: When::Timed { start, end },
      calendars: vec![],
      location: None,
      recurring: false,
      response: None,
      cancelled: false,
    })
  }

  struct Case {
    period: Period,
    notes: Vec<Notes>,
    calendar: Option<Vec<Event>>,
    tags: BTreeMap<String, String>,
    bill: BTreeMap<String, f64>,
    rules: Rules,
  }

  impl Case {
    fn new(period: Period, notes: Vec<Notes>) -> Case {
      Case {
        period,
        notes,
        calendar: None,
        tags: BTreeMap::new(),
        bill: BTreeMap::new(),
        rules: Rules::default(),
      }
    }

    fn build(&self) -> Data {
      self.build_on(self.period.days().1)
    }

    fn build_on(&self, today: Date) -> Data {
      build(&Inputs {
        period: self.period,
        notes: &self.notes,
        calendar: self.calendar.as_deref(),
        tags: &self.tags,
        bill: &self.bill,
        rules: &self.rules,
        today,
      })
    }
  }

  fn day(data: &Data, date: Date) -> Option<&Day> {
    data.days.iter().find(|d| d.date == date)
  }

  fn kinds(data: &Data, date: Option<Date>) -> Vec<&'static str> {
    data
      .flags
      .iter()
      .filter(|f| date.is_none() || f.date == date)
      .map(|f| f.kind)
      .collect()
  }

  #[test]
  fn grid_codes_are_floored_digits_then_letters() {
    assert_eq!(grid_code(0.0), '0');
    assert_eq!(grid_code(9.99), '9');
    assert_eq!(grid_code(10.0), 'A');
    assert_eq!(grid_code(20.5), 'K');
    assert_eq!(grid_code(21.0), '+');
  }

  #[test]
  fn tags_match_in_any_case_and_take_the_configured_spelling() -> Result<()> {
    let date = Date::new(2026, 10, 5)?;
    let rows: &[&str] = &[
      "| @Acme | 09:00-10:00 | 1.0 | a |",
      "| @acme | 10:00-11:00 | 1.0 | b |",
      "| @acmee | 11:00-12:00 | 1.0 | typo |",
    ];
    let mut case = Case::new(Period::Week(date), vec![month_file(date, &[(date, rows)])]);
    case.tags.insert("@acmee".into(), "@Acme".into());
    let data = case.build();
    let d = day(&data, date).ok_or("day")?;
    assert_eq!(d.totals.by_tag.keys().collect::<Vec<_>>(), vec!["@Acme"]);
    assert_eq!(d.totals.hours, rows.len() as f64);
    assert!(kinds(&data, None).contains(&"tag_spellings"));
    Ok(())
  }

  #[test]
  fn excluded_tags_and_titles_count_nothing() -> Result<()> {
    let date = Date::new(2026, 10, 5)?;
    let rows: &[&str] = &[
      "| @acme | 09:00-17:00 | 8.0 | work |",
      "| @self | 17:00-18:00 | 1.0 | errands |",
      "| @acme | 18:00-19:00 | 1.0 | [GYM] run |",
    ];
    let mut case = Case::new(Period::Week(date), vec![month_file(date, &[(date, rows)])]);
    case.rules.exclude = vec!["@Self".into(), "[gym]".into()];
    let data = case.build();
    let d = day(&data, date).ok_or("day")?;
    assert_eq!(d.totals.hours, 8.0);
    assert_eq!(d.entries.iter().filter(|e| !e.counted).count(), 2);
    Ok(())
  }

  #[test]
  fn weekend_tags_are_flagged_not_moved() -> Result<()> {
    let saturday = Date::new(2026, 10, 10)?;
    let rows: &[&str] = &["| @acme | 10:00-12:00 | 2.0 | x |"];
    let mut case = Case::new(
      Period::Week(saturday),
      vec![month_file(saturday, &[(saturday, rows)])],
    );
    case.rules.weekday_only = vec!["@ACME".into()];
    let data = case.build();
    assert_eq!(day(&data, saturday).map(|d| d.totals.hours), Some(2.0));
    assert!(kinds(&data, Some(saturday)).contains(&"weekend_tag"));
    Ok(())
  }

  #[test]
  fn a_session_past_midnight_counts_on_the_day_it_started() -> Result<()> {
    let date = Date::new(2026, 10, 6)?;
    let next = date.saturating_add(1.day());
    let rows: &[&str] = &["| @a | 21:30-03:00 | 5.5 | late |"];
    let case = Case::new(Period::Week(date), vec![month_file(date, &[(date, rows)])]);
    let data = case.build();
    assert_eq!(day(&data, date).map(|d| d.totals.hours), Some(5.5));
    assert_eq!(day(&data, next).map(|d| d.totals.hours), Some(0.0));
    Ok(())
  }

  #[test]
  fn a_block_after_midnight_joins_the_session_before_it_across_a_month() -> Result<()> {
    let last = Date::new(2026, 10, 31)?;
    let first = last.saturating_add(1.day());
    let october = month_file(last, &[(last, &["| @a | 22:00-00:00 | 2.0 | evening |"])]);
    let november = month_file(
      first,
      &[(
        first,
        &[
          "| @a | 00:00-01:00 | 1.0 | still going |",
          "| @a | 01:00-02:00 | 1.0 | and on |",
          "| @a | 09:00-10:00 | 1.0 | morning |",
        ],
      )],
    );
    let data = Case::new(Period::Week(last), vec![october, november]).build();
    let before = day(&data, last).ok_or("day")?;
    assert_eq!(before.totals.hours, 4.0);
    assert!(
      before
        .entries
        .iter()
        .filter(|e| e.carried_from == Some(first))
        .count()
        == 2
    );
    assert_eq!(day(&data, first).map(|d| d.totals.hours), Some(1.0));
    Ok(())
  }

  #[test]
  fn a_block_at_midnight_after_a_gap_stays_on_its_own_day() -> Result<()> {
    let date = Date::new(2026, 10, 6)?;
    let next = date.saturating_add(1.day());
    let notes = month_file(
      date,
      &[
        (date, &["| @a | 21:00-23:00 | 2.0 | x |"]),
        (next, &["| @a | 00:00-01:00 | 1.0 | y |"]),
      ],
    );
    let data = Case::new(Period::Week(date), vec![notes]).build();
    assert_eq!(day(&data, date).map(|d| d.totals.hours), Some(2.0));
    assert_eq!(day(&data, next).map(|d| d.totals.hours), Some(1.0));
    Ok(())
  }

  #[test]
  fn the_calendar_fills_days_without_notes_counting_overlaps_once() -> Result<()> {
    let noted = Date::new(2026, 10, 5)?;
    let bare = noted.saturating_add(1.day());
    let notes = month_file(noted, &[(noted, &["| @a | 09:00-17:00 | 8.0 | x |"])]);
    let mut case = Case::new(Period::Week(noted), vec![notes]);
    case.rules.zero_hours = vec!["[Travel]".into()];
    case.calendar = Some(vec![
      event("[A] busy", noted, (9, 0), 60)?,
      event("[A] workshop", bare, (9, 0), 120)?,
      event("[B] overlapping call", bare, (10, 0), 120)?,
      event("[Travel] train", bare, (13, 0), 60)?,
      event("Lunch", bare, (12, 0), 60)?,
    ]);
    let data = case.build();
    let d = day(&data, bare).ok_or("day")?;
    assert_eq!(d.source, Some(Source::Calendar));
    // 09:00 to 12:00 net; the hold and the untagged lunch count nothing.
    assert_eq!(d.totals.hours, 3.0);
    assert_eq!(d.totals.by_tag.get("@a"), Some(&2.0));
    assert_eq!(d.totals.by_tag.get("@b"), Some(&1.0));
    let flags = kinds(&data, Some(bare));
    assert!(flags.contains(&"calendar_overlap"));
    assert!(flags.contains(&"calendar_only"));
    let n = day(&data, noted).ok_or("day")?;
    assert_eq!(n.source, Some(Source::Notes));
    assert_eq!(n.totals.hours, 8.0);
    assert!(kinds(&data, Some(noted)).contains(&"sources_differ"));
    Ok(())
  }

  #[test]
  fn rows_that_disagree_with_themselves_are_flagged() -> Result<()> {
    let date = Date::new(2026, 10, 5)?;
    let rows: &[&str] = &[
      "| @a | 09:00-11:00 | 1.0 | short by an hour |",
      "| @a | 11:00-12:00 |     | range only |",
      "| @a |             |     | nothing |",
      "|    |             | 9.0 |  |",
    ];
    let data = Case::new(Period::Week(date), vec![month_file(date, &[(date, rows)])]).build();
    let flags = kinds(&data, Some(date));
    for kind in ["hours_disagree", "no_hours", "total_disagrees"] {
      assert!(flags.contains(&kind), "{kind} in {flags:?}");
    }
    assert_eq!(day(&data, date).map(|d| d.totals.hours), Some(2.0));
    Ok(())
  }

  #[test]
  fn a_month_summarises_the_weeks_that_start_in_it() -> Result<()> {
    let wednesday = Date::new(2026, 10, 28)?;
    let sunday = wednesday.saturating_add(4.days());
    let october = month_file(
      wednesday,
      &[(wednesday, &["| @a | 09:00-10:00 | 1.0 | x |"])],
    );
    let november = month_file(sunday, &[(sunday, &["| @a | 09:00-11:00 | 2.0 | y |"])]);
    let data = Case::new(Period::Month(wednesday), vec![october, november]).build();
    let month = data.months.first().ok_or("month")?;
    assert_eq!(month.totals.hours, 1.0);
    assert_eq!(
      month.grid.chars().count(),
      usize::from(wednesday.days_in_month() as u8)
    );
    let spanning = data.weeks.iter().find(|w| w.last == sunday).ok_or("week")?;
    assert_eq!(spanning.totals.hours, 3.0);
    assert!(month.weeks.contains(&spanning.week));
    let first_week = data.weeks.first().ok_or("week")?;
    assert!(first_week.first < wednesday.first_of_month());
    assert!(!month.weeks.contains(&first_week.week));
    Ok(())
  }

  #[test]
  fn the_file_whose_name_covers_a_day_wins_and_a_different_copy_is_flagged() -> Result<()> {
    let date = Date::new(2026, 11, 1)?;
    let previous = month_file(
      date.saturating_sub(1.day()),
      &[(date, &["| @a | 09:00-10:00 | 1.0 | early copy |"])],
    );
    let own = month_file(date, &[(date, &["| @a | 09:00-12:00 | 3.0 | final |"])]);
    let data = Case::new(Period::Week(date), vec![previous, own]).build();
    assert_eq!(day(&data, date).map(|d| d.totals.hours), Some(3.0));
    assert!(kinds(&data, Some(date)).contains(&"day_differs"));
    Ok(())
  }

  #[test]
  fn days_after_today_take_nothing_from_the_calendar() -> Result<()> {
    let today = Date::new(2026, 11, 1)?;
    let tomorrow = today.saturating_add(1.day());
    let mut case = Case::new(Period::Week(tomorrow), vec![]);
    case.calendar = Some(vec![event("[A] booked", tomorrow, (9, 0), 60)?]);
    let data = case.build_on(today);
    let d = day(&data, tomorrow).ok_or("day")?;
    assert_eq!(d.source, None);
    assert_eq!(d.totals.hours, 0.0);
    assert!(kinds(&data, Some(tomorrow)).is_empty());
    Ok(())
  }

  #[test]
  fn billable_hours_take_the_row_s_bill_else_the_tag_s() -> Result<()> {
    let date = Date::new(2026, 11, 2)?;
    let text = format!(
      "#### {} {}\n\n#### Time Log\n\n| Project | Time | Hours | Bill | What |\n|---|---|---|---|---|\n\
       | @a | 08:00-10:00 | 2.0 | 0.5 | travel |\n\
       | @a | 10:00-16:00 | 6.0 |     | work |\n\
       | @b | 16:00-17:00 | 1.0 |     | own time |\n\
       | @x | 17:00-18:00 | 1.0 | 1.0 | excluded, so not billed |\n",
      date.strftime("%Y%m%d"),
      weekday_name(date.weekday())
    );
    let notes = notes::parse(
      &text,
      Path::new(&crate::names::month_file(date, crate::names::DAY_NOTES)),
    );
    let mut case = Case::new(Period::Week(date), vec![notes]);
    let full = 1.0;
    case.bill.insert("@A".into(), full);
    case.rules.exclude = vec!["@x".into()];
    let data = case.build();
    let d = day(&data, date).ok_or("day")?;
    let travel = 2.0 * 0.5;
    let work = 6.0 * full;
    assert_eq!(d.totals.billable_by_tag.get("@a"), Some(&(travel + work)));
    assert_eq!(d.totals.billable_by_tag.get("@b"), None);
    assert_eq!(d.totals.billable, travel + work);
    assert_eq!(d.totals.hours, 9.0);
    let week = data
      .weeks
      .iter()
      .find(|w| w.first <= date && date <= w.last)
      .ok_or("week")?;
    assert_eq!(week.totals.billable, d.totals.billable);
    Ok(())
  }

  #[test]
  fn overlapping_rows_are_flagged() -> Result<()> {
    let date = Date::new(2026, 8, 25)?;
    let rows: &[&str] = &[
      "| @a | 15:00-15:30 | 0.5 | call |",
      "| @a | 15:00-18:00 | 3.0 | work |",
      "| @a | 18:00-19:00 | 1.0 | after, touching |",
    ];
    let data = Case::new(Period::Week(date), vec![month_file(date, &[(date, rows)])]).build();
    let overlaps = kinds(&data, Some(date))
      .into_iter()
      .filter(|k| *k == "notes_overlap")
      .count();
    assert_eq!(overlaps, 1);
    Ok(())
  }
}
