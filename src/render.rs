//! Generated sections in the Day Notes. cldr writes only between markers it owns:
//!
//! - `<!-- cldr:month 2026-10 -->` ... `<!-- /cldr:month -->`, under the file's title: the month
//!   grid and the month's hours by tag.
//! - `<!-- cldr:week 2026-W44 -->` ... `<!-- /cldr:week -->`, directly under the week's heading
//!   (`#### 20261026 (Week 44)`): the week's hours by tag and day. A week is summarised in the
//!   file of the month it starts in.
//!
//! Everything outside the markers is kept byte for byte. A week with no heading gets one, placed
//! in the file's newest-first order. Rendering twice changes nothing.

use crate::model::{Data, Day, Week, round, weekday_name};
use crate::period::Period;
use jiff::ToSpan;
use jiff::civil::Date;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The level of the Day Notes' day and week headings (`####`).
const HEADING_LEVEL: usize = 4;
const END_MONTH: &str = "<!-- /cldr:month -->";
const END_WEEK: &str = "<!-- /cldr:week -->";

fn start_month(id: &str) -> String {
  format!("<!-- cldr:month {id} -->")
}

fn start_week(id: &str) -> String {
  format!("<!-- cldr:week {id} -->")
}

fn hours(h: f64) -> String {
  if h == 0.0 {
    String::new()
  } else if (h * 10.0).fract() == 0.0 {
    format!("{h:.1}")
  } else {
    format!("{h:.2}")
  }
}

/// A Markdown table, columns padded; the first column left, the rest right.
fn table(header: &[String], rows: &[Vec<String>], total: &[String]) -> String {
  let mut owned: Vec<Vec<String>> = vec![header.to_vec()];
  owned.extend(rows.iter().cloned());
  owned.push(total.to_vec());
  let width: Vec<usize> = (0..header.len())
    .map(|k| {
      owned
        .iter()
        .map(|r| r[k].chars().count())
        .max()
        .unwrap_or(0)
    })
    .collect();
  let line = |r: &[String]| {
    let cells: Vec<String> = r
      .iter()
      .enumerate()
      .map(|(k, c)| {
        if k == 0 {
          format!("{c:<w$}", w = width[k])
        } else {
          format!("{c:>w$}", w = width[k])
        }
      })
      .collect();
    format!("| {} |", cells.join(" | "))
  };
  let rule: String = width
    .iter()
    .enumerate()
    .map(|(k, w)| {
      if k == 0 {
        "-".repeat(w + 2)
      } else {
        format!("{}:", "-".repeat(w + 1))
      }
    })
    .collect::<Vec<_>>()
    .join("|");
  let mut out = String::new();
  let _ = writeln!(out, "{}", line(header));
  let _ = writeln!(out, "|{rule}|");
  for r in rows {
    let _ = writeln!(out, "{}", line(r));
  }
  let _ = writeln!(out, "|{rule}|");
  let _ = writeln!(out, "{}", line(total));
  out
}

/// The month grid, as the old Activity Log drew it: weekday initials, the day number in two
/// rows, then each day's code; days after today are left blank.
pub fn month_grid(month: Date, days: &[&Day], today: Date) -> String {
  let (first, last) = Period::Month(month).days();
  let dates: Vec<Date> = first.series(1.day()).take_while(|d| *d <= last).collect();
  let row = |f: &dyn Fn(Date) -> char| -> String {
    dates
      .iter()
      .map(|d| f(*d).to_string())
      .collect::<Vec<_>>()
      .join(" ")
  };
  let initial = |d: Date| weekday_name(d.weekday()).chars().next().unwrap_or(' ');
  let tens = |d: Date| match d.day() / 10 {
    0 => ' ',
    n => char::from_digit(n as u32, 10).unwrap_or(' '),
  };
  let units = |d: Date| char::from_digit((d.day() % 10) as u32, 10).unwrap_or(' ');
  let code = |d: Date| {
    if d > today {
      ' '
    } else {
      days.iter().find(|x| x.date == d).map_or('0', |x| x.code)
    }
  };
  let rule = "-".repeat(dates.len() * 2 - 1);
  let mut out = String::new();
  let _ = writeln!(out, "```text");
  let _ = writeln!(out, "{}", month.strftime("%B %Y"));
  let _ = writeln!(out, "{}", row(&initial));
  let _ = writeln!(out, "{}", row(&tens).trim_end());
  let _ = writeln!(out, "{}", row(&units));
  let _ = writeln!(out, "{rule}");
  let _ = writeln!(out, "{}", row(&code).trim_end());
  let _ = writeln!(out, "{rule}");
  let _ = writeln!(out, "```");
  out
}

fn tag_table(
  by_tag: &std::collections::BTreeMap<String, f64>,
  billable: &std::collections::BTreeMap<String, f64>,
) -> String {
  let header: Vec<String> = ["Project", "Hours", "Billable"].map(String::from).to_vec();
  let rows: Vec<Vec<String>> = by_tag
    .iter()
    .map(|(t, h)| {
      vec![
        t.clone(),
        hours(*h),
        hours(billable.get(t).copied().unwrap_or(0.0)),
      ]
    })
    .collect();
  let total = vec![
    "Total".to_owned(),
    hours(round(by_tag.values().sum())),
    hours(round(billable.values().sum())),
  ];
  table(&header, &rows, &total)
}

/// The month section's body.
pub fn month_body(data: &Data, month: Date, today: Date) -> String {
  let (first, last) = Period::Month(month).days();
  let days: Vec<&Day> = data
    .days
    .iter()
    .filter(|d| d.date >= first && d.date <= last)
    .collect();
  let mut by_tag = std::collections::BTreeMap::new();
  let mut billable = std::collections::BTreeMap::new();
  for d in &days {
    for (t, h) in &d.totals.by_tag {
      *by_tag.entry(t.clone()).or_insert(0.0) += h;
    }
    for (t, h) in &d.totals.billable_by_tag {
      *billable.entry(t.clone()).or_insert(0.0) += h;
    }
  }
  let by_tag = by_tag.into_iter().map(|(t, h)| (t, round(h))).collect();
  let billable = billable.into_iter().map(|(t, h)| (t, round(h))).collect();
  format!(
    "{}\n{}",
    month_grid(month, &days, today),
    tag_table(&by_tag, &billable)
  )
}

/// The week section's body: hours by tag and day, as the old Activity Log's week summary.
pub fn week_body(data: &Data, week: &Week) -> String {
  let dates: Vec<Date> = week.first.series(1.day()).take(7).collect();
  let day = |d: Date| data.days.iter().find(|x| x.date == d);
  let mut header = vec!["Project".to_owned()];
  header.extend(
    dates
      .iter()
      .map(|d| weekday_name(d.weekday())[..3].to_owned()),
  );
  header.push("Total".to_owned());
  let tags: BTreeSet<&String> = week.totals.by_tag.keys().collect();
  let rows: Vec<Vec<String>> = tags
    .iter()
    .map(|tag| {
      let mut r = vec![(*tag).clone()];
      r.extend(dates.iter().map(|d| {
        hours(
          day(*d)
            .and_then(|x| x.totals.by_tag.get(*tag))
            .copied()
            .unwrap_or(0.0),
        )
      }));
      r.push(hours(week.totals.by_tag.get(*tag).copied().unwrap_or(0.0)));
      r
    })
    .collect();
  let mut total = vec!["Total".to_owned()];
  total.extend(dates.iter().map(|d| {
    let h = day(*d).map_or(0.0, |x| x.totals.hours);
    if h == 0.0 { "0.0".to_owned() } else { hours(h) }
  }));
  total.push(hours(week.totals.hours));
  let mut out = table(&header, &rows, &total);
  if week.totals.billable > 0.0 {
    let _ = writeln!(out, "\nBillable: {} h", hours(week.totals.billable));
  }
  out
}

/// Replace the body between a start marker and its end, or `None` when the start is absent.
fn replace_section(text: &str, start: &str, end: &str, body: &str) -> Option<String> {
  let a = text.find(start)?;
  let after_start = a + start.len();
  let b = after_start + text[after_start..].find(end)?;
  Some(format!("{}\n{}{}", &text[..after_start], body, &text[b..]))
}

fn section(start: &str, body: &str, end: &str) -> String {
  format!("{start}\n{body}{end}\n")
}

/// A line's date and kind: a week heading `#### YYYYMMDD (Week NN)` or a day heading.
fn heading_date(line: &str) -> Option<(Date, bool)> {
  // Day and week headings are level four or deeper; the file's title also starts with a date.
  let rest = line.trim_start_matches('#');
  if line.len() - rest.len() < HEADING_LEVEL {
    return None;
  }
  let rest = rest.trim();
  let digits = rest.get(0..8)?;
  if !digits.chars().all(|c| c.is_ascii_digit()) {
    return None;
  }
  let date = format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8])
    .parse::<Date>()
    .ok()?;
  Some((date, rest[8..].trim_start().starts_with("(Week")))
}

/// Insert or refresh a week's section in the text.
fn put_week(text: &str, week: &Week, body: &str) -> String {
  let start = start_week(&week.week);
  if let Some(t) = replace_section(text, &start, END_WEEK, body) {
    return t;
  }
  let block = section(&start, body, END_WEEK);
  // Under the week's heading, when there is one.
  let mut offset = 0;
  for line in text.split_inclusive('\n') {
    if let Some((date, true)) = heading_date(line)
      && date == week.first
    {
      let at = offset + line.len();
      return format!("{}\n{block}{}", &text[..at], &text[at..]);
    }
    offset += line.len();
  }
  // Otherwise a new heading, before the first heading of anything older (newest first).
  let heading = format!(
    "#### {} (Week {:02})\n\n{block}\n",
    week.first.strftime("%Y%m%d"),
    week.first.iso_week_date().week()
  );
  // A block opens at a `#### ---` rule or a TWIL heading just above a dated heading.
  let mut offset = 0;
  let mut block_start: Option<usize> = None;
  for line in text.split_inclusive('\n') {
    let t = line.trim();
    if t.starts_with("#### -") || t.starts_with("##### This Week I Learned") {
      block_start.get_or_insert(offset);
    } else if let Some((date, _)) = heading_date(line) {
      if date <= week.last {
        let at = block_start.unwrap_or(offset);
        return format!("{}{heading}{}", &text[..at], &text[at..]);
      }
      block_start = None;
    } else if !t.is_empty() {
      block_start = None;
    }
    offset += line.len();
  }
  let sep = if text.ends_with('\n') { "" } else { "\n" };
  format!("{text}{sep}\n{heading}")
}

/// Insert or refresh the month section, under the file's `# ` title (or at the top).
fn put_month(text: &str, id: &str, body: &str) -> String {
  let start = start_month(id);
  if let Some(t) = replace_section(text, &start, END_MONTH, body) {
    return t;
  }
  let block = section(&start, body, END_MONTH);
  let mut offset = 0;
  for line in text.split_inclusive('\n') {
    if line.starts_with("# ") {
      let at = offset + line.len();
      return format!("{}\n{block}{}", &text[..at], &text[at..]);
    }
    offset += line.len();
  }
  format!("{block}\n{text}")
}

/// What to render into one month's Day Notes.
pub enum Scope<'a> {
  Month,
  Week(&'a str),
}

/// The Day Notes text with its generated sections rendered: the month section, and the sections
/// of the weeks that start in the month (or only the one week asked for).
pub fn render(text: &str, month: Date, data: &Data, today: Date, scope: Scope<'_>) -> String {
  let month = month.first_of_month();
  let id = month.strftime("%Y-%m").to_string();
  let mut out = put_month(text, &id, &month_body(data, month, today));
  for week in data
    .weeks
    .iter()
    .filter(|w| w.first.first_of_month() == month)
  {
    if let Scope::Week(only) = scope
      && week.week != only
    {
      continue;
    }
    out = put_week(&out, week, &week_body(data, week));
  }
  out
}

/// Every generated section removed, markers included: the hand-written text alone.
pub fn strip(text: &str) -> String {
  let mut out = String::new();
  let mut skipping = false;
  for line in text.split_inclusive('\n') {
    let t = line.trim();
    if t.starts_with("<!-- cldr:") {
      skipping = true;
      continue;
    }
    if t.starts_with("<!-- /cldr:") {
      skipping = false;
      continue;
    }
    if !skipping {
      out.push_str(line);
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Rules;
  use crate::model::{self, Inputs};
  use crate::notes;
  use std::collections::BTreeMap;
  use std::path::Path;

  const HEADER: &str = "| Project | Time | Hours | What |\n|---|:---:|---:|---|";

  /// A month's Day Notes as the user writes them: title, then week blocks, newest first.
  fn day_notes(month: Date, weeks_with_headings: &[Date], days: &[(Date, &str)]) -> String {
    let mut text = format!(
      "---\nverblock: \"x\"\n---\n# {} Day Notes\n\n|-----|\n",
      crate::month::range(month)
    );
    let mut items: Vec<(Date, String)> = Vec::new();
    for monday in weeks_with_headings {
      items.push((
        monday.saturating_add(7.days()),
        format!(
          "\n##### This Week I Learned #TWIL\n\n#### {} (Week {:02})\n\nSomething learned.\n\n",
          monday.strftime("%Y%m%d"),
          monday.iso_week_date().week()
        ),
      ));
    }
    for (date, row) in days {
      items.push((
        *date,
        format!(
          "#### ---------------\n#### {} {}\n\n#### Time Log\n\n{HEADER}\n{row}\n\nNotes for the day.\n\n",
          date.strftime("%Y%m%d"),
          weekday_name(date.weekday())
        ),
      ));
    }
    items.sort_by_key(|i| std::cmp::Reverse(i.0));
    for (_, block) in items {
      text.push_str(&block);
    }
    text
  }

  fn data_for(month: Date, text: &str) -> Data {
    let name = crate::names::month_file(month, crate::names::DAY_NOTES);
    let parsed = notes::parse(text, Path::new(&name));
    let (tags, bill, rules) = (BTreeMap::new(), BTreeMap::new(), Rules::default());
    model::build(&Inputs {
      period: Period::Month(month),
      notes: &[parsed],
      calendar: None,
      tags: &tags,
      bill: &bill,
      rules: &rules,
      today: Period::Month(month).days().1,
    })
  }

  fn case() -> crate::Result<(Date, String)> {
    let month = Date::new(2026, 10, 1)?;
    let mondays = [Date::new(2026, 10, 19)?, Date::new(2026, 10, 12)?];
    let days = [
      (
        Date::new(2026, 10, 20)?,
        "| @a | 09:00-17:00 | 8.0 | work |",
      ),
      (
        Date::new(2026, 10, 13)?,
        "| @a | 09:00-12:00 | 3.0 | work |",
      ),
      (
        Date::new(2026, 10, 6)?,
        "| @b | 09:00-10:00 | 1.0 | other |",
      ),
    ];
    Ok((month, day_notes(month, &mondays, &days)))
  }

  #[test]
  fn rendering_keeps_the_hand_written_text_and_twice_changes_nothing() -> crate::Result<()> {
    let (month, text) = case()?;
    let data = data_for(month, &text);
    let today = Period::Month(month).days().1;
    let once = render(&text, month, &data, today, Scope::Month);
    let twice = render(&once, month, &data, today, Scope::Month);
    assert_eq!(once, twice);
    assert_ne!(once, text);
    // Stripped of its sections, only the headings cldr added remain beyond the original.
    let stripped = strip(&once);
    for line in text.lines() {
      assert!(stripped.contains(line), "lost: {line:?}");
    }
    Ok(())
  }

  #[test]
  fn each_week_starting_in_the_month_gets_a_section_under_its_heading() -> crate::Result<()> {
    let (month, text) = case()?;
    let data = data_for(month, &text);
    let out = render(
      &text,
      month,
      &data,
      Period::Month(month).days().1,
      Scope::Month,
    );
    let in_month: Vec<&Week> = data
      .weeks
      .iter()
      .filter(|w| w.first.first_of_month() == month)
      .collect();
    for week in &in_month {
      let marker = start_week(&week.week);
      assert_eq!(out.matches(&marker).count(), 1, "{marker}");
    }
    // A week that starts in the month before is not summarised here.
    let earlier = data.weeks.iter().find(|w| w.first < month).ok_or("week")?;
    assert!(!out.contains(&start_week(&earlier.week)));
    // A heading the user wrote is followed by its section.
    let heading = format!("#### {} (Week", Date::new(2026, 10, 19)?.strftime("%Y%m%d"));
    let at = out.find(&heading).ok_or("heading")?;
    let next = &out[at..];
    assert!(
      next
        .split('\n')
        .nth(2)
        .is_some_and(|l| l.starts_with("<!-- cldr:week"))
    );
    Ok(())
  }

  #[test]
  fn a_week_without_a_heading_gets_one_in_newest_first_order() -> crate::Result<()> {
    let (month, text) = case()?;
    let data = data_for(month, &text);
    let out = render(
      &text,
      month,
      &data,
      Period::Month(month).days().1,
      Scope::Month,
    );
    let monday = Date::new(2026, 10, 5)?;
    let heading = format!(
      "#### {} (Week {:02})",
      monday.strftime("%Y%m%d"),
      monday.iso_week_date().week()
    );
    let at = out.find(&heading).ok_or("new heading")?;
    let newer_day = out.find("#### 20261013").ok_or("day")?;
    let its_day = out.find("#### 20261006").ok_or("day")?;
    assert!(newer_day < at && at < its_day);
    Ok(())
  }

  #[test]
  fn the_month_grid_has_a_code_per_day_to_today() -> crate::Result<()> {
    let (month, text) = case()?;
    let data = data_for(month, &text);
    let today = Date::new(2026, 10, 20)?;
    let out = render(&text, month, &data, today, Scope::Month);
    let codes = month_grid(month, &data.days.iter().collect::<Vec<_>>(), today);
    let line = codes.lines().nth(6).ok_or("codes")?;
    let written = line.split(' ').filter(|c| !c.is_empty()).count();
    assert_eq!(written, usize::try_from(today.day())?);
    assert!(out.contains(&start_month("2026-10")));
    Ok(())
  }

  #[test]
  fn a_changed_week_replaces_only_its_own_section() -> crate::Result<()> {
    let (month, text) = case()?;
    let today = Period::Month(month).days().1;
    let once = render(&text, month, &data_for(month, &text), today, Scope::Month);
    let edited = once.replace(
      "| @a | 09:00-17:00 | 8.0 | work |",
      "| @a | 09:00-18:00 | 9.0 | work |",
    );
    let again = render(
      &edited,
      month,
      &data_for(month, &edited),
      today,
      Scope::Month,
    );
    assert_eq!(strip(&again), strip(&edited));
    assert!(again.contains("9.0"));
    Ok(())
  }
}
