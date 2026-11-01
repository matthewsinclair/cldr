//! Repeating events: a rule, expanded into the occurrences inside a window.
//!
//! Weekday numbers follow the store: 0 is Sunday.

use crate::Result;
use jiff::civil::Date;
use jiff::{Timestamp, ToSpan, Zoned};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Freq {
  #[default]
  Daily,
  Weekly,
  Monthly,
  Yearly,
}

impl Freq {
  pub fn from_store(value: i64) -> Option<Freq> {
    match value {
      0 => Some(Freq::Daily),
      1 => Some(Freq::Weekly),
      2 => Some(Freq::Monthly),
      3 => Some(Freq::Yearly),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct Rule {
  pub freq: Freq,
  /// At least 1.
  pub interval: i64,
  /// 0 for no limit.
  pub count: i64,
  /// Inclusive.
  pub until: Option<Timestamp>,
  pub by_day: Vec<i64>,
  /// The ordinal for each `by_day` entry (eg 2 for "the second"), 0 for every one.
  pub by_day_ordinal: Vec<i64>,
  pub by_month_day: Vec<i64>,
  pub by_month: Vec<i64>,
  pub by_set_pos: Vec<i64>,
  pub week_start: i64,
  /// Original starts, in whole seconds, that do not happen.
  pub exceptions: BTreeSet<i64>,
  /// Extra starts outside the rule.
  pub extra: Vec<Timestamp>,
}

pub fn store_weekday(date: Date) -> i64 {
  i64::from(date.weekday().to_sunday_zero_offset())
}

fn add_days(date: Date, days: i64) -> Date {
  date.saturating_add(days.days())
}

fn month_days(year: i16, month: i8, rule: &Rule, anchor: Date) -> Vec<Date> {
  let Ok(first) = Date::new(year, month, 1) else {
    return Vec::new();
  };
  let length = i64::from(first.days_in_month());
  let day = |n: i64| {
    i8::try_from(n)
      .ok()
      .and_then(|n| Date::new(year, month, n).ok())
  };
  let mut days = Vec::new();
  if !rule.by_month_day.is_empty() {
    for &n in &rule.by_month_day {
      let n = if n > 0 { n } else { length + 1 + n };
      if (1..=length).contains(&n) {
        days.extend(day(n));
      }
    }
  } else if !rule.by_day.is_empty() {
    let span: Vec<Date> = (0..length).map(|i| add_days(first, i)).collect();
    for (i, &weekday) in rule.by_day.iter().enumerate() {
      let ordinal = rule
        .by_day_ordinal
        .get(i)
        .or(rule.by_day_ordinal.first())
        .copied()
        .unwrap_or(0);
      let hits: Vec<Date> = span
        .iter()
        .copied()
        .filter(|d| store_weekday(*d) == weekday.rem_euclid(7))
        .collect();
      let count = i64::try_from(hits.len()).unwrap_or(0);
      if ordinal == 0 {
        days.extend(hits);
      } else if (-count..=count).contains(&ordinal) {
        let index = if ordinal > 0 {
          ordinal - 1
        } else {
          count + ordinal
        };
        days.extend(
          usize::try_from(index)
            .ok()
            .and_then(|i| hits.get(i))
            .copied(),
        );
      }
    }
  } else if i64::from(anchor.day()) <= length {
    days.extend(day(i64::from(anchor.day())));
  }
  days.sort();
  days.dedup();
  if !rule.by_set_pos.is_empty() {
    let count = i64::try_from(days.len()).unwrap_or(0);
    days = rule
      .by_set_pos
      .iter()
      .filter(|&&p| p != 0 && (-count..=count).contains(&p))
      .filter_map(|&p| usize::try_from(if p > 0 { p - 1 } else { count + p }).ok())
      .filter_map(|i| days.get(i).copied())
      .collect();
  }
  days
}

/// The dates the rule produces from `anchor` until `stop`, in order.
pub fn candidate_days(rule: &Rule, anchor: Date, stop: Date) -> Vec<Date> {
  let step = rule.interval.max(1);
  let mut out = Vec::new();
  match rule.freq {
    Freq::Daily => {
      let mut d = anchor;
      while d <= stop {
        let weekday_ok = rule.by_day.is_empty() || rule.by_day.contains(&store_weekday(d));
        let month_ok = rule.by_month.is_empty() || rule.by_month.contains(&i64::from(d.month()));
        let day_ok =
          rule.by_month_day.is_empty() || rule.by_month_day.contains(&i64::from(d.day()));
        if weekday_ok && month_ok && day_ok {
          out.push(d);
        }
        d = add_days(d, step);
      }
    }
    Freq::Weekly => {
      let weekdays = if rule.by_day.is_empty() {
        vec![store_weekday(anchor)]
      } else {
        rule.by_day.clone()
      };
      let back = (store_weekday(anchor) - rule.week_start).rem_euclid(7);
      let mut week = add_days(anchor, -back);
      while week <= stop {
        for offset in 0..7 {
          let d = add_days(week, offset);
          if weekdays.contains(&store_weekday(d)) {
            out.push(d);
          }
        }
        week = add_days(week, 7 * step);
      }
    }
    Freq::Monthly => {
      let (mut year, mut month) = (i64::from(anchor.year()), i64::from(anchor.month()));
      while let (Ok(y), Ok(m)) = (i16::try_from(year), i8::try_from(month)) {
        match Date::new(y, m, 1) {
          Ok(start) if start <= stop => out.extend(month_days(y, m, rule, anchor)),
          _ => break,
        }
        month += step;
        year += (month - 1).div_euclid(12);
        month = (month - 1).rem_euclid(12) + 1;
      }
    }
    Freq::Yearly => {
      let mut year = i64::from(anchor.year());
      while let Ok(y) = i16::try_from(year) {
        match Date::new(y, 1, 1) {
          Ok(start) if start <= stop => {}
          _ => break,
        }
        let months = if rule.by_month.is_empty() {
          vec![i64::from(anchor.month())]
        } else {
          rule.by_month.clone()
        };
        for month in months {
          let Ok(m) = i8::try_from(month) else { continue };
          if !rule.by_month_day.is_empty() || !rule.by_day.is_empty() {
            out.extend(month_days(y, m, rule, anchor));
          } else if let Ok(d) = Date::new(y, m, anchor.day()) {
            out.push(d);
          }
        }
        year += step;
      }
    }
  }
  out
}

/// The occurrences of a repeating event that overlap `[window_start, window_end)`, as (start, end).
/// `moved` holds the original starts, in whole seconds, of occurrences that were changed and stored apart.
pub fn expand(
  start: &Zoned,
  end: &Zoned,
  rule: &Rule,
  moved: &BTreeSet<i64>,
  window_start: Timestamp,
  window_end: Timestamp,
) -> Result<Vec<(Timestamp, Timestamp)>> {
  let zone = start.time_zone().clone();
  let length = end.timestamp().as_second() - start.timestamp().as_second();
  let wall = start.time();
  let stop = add_days(window_end.to_zoned(zone.clone()).date(), 1);
  let mut out = Vec::new();
  let mut seen = 0;
  let span = |from: Timestamp| -> Result<(Timestamp, Timestamp)> {
    Ok((from, Timestamp::from_second(from.as_second() + length)?))
  };
  for d in candidate_days(rule, start.date(), stop) {
    if d < start.date() {
      continue;
    }
    let occurrence = d.to_datetime(wall).to_zoned(zone.clone())?.timestamp();
    if occurrence < start.timestamp() {
      continue;
    }
    if rule.until.is_some_and(|until| occurrence > until) {
      break;
    }
    seen += 1;
    if rule.count > 0 && seen > rule.count {
      break;
    }
    let key = occurrence.as_second();
    if rule.exceptions.contains(&key) || moved.contains(&key) {
      continue;
    }
    let (from, to) = span(occurrence)?;
    if to > window_start && from < window_end {
      out.push((from, to));
    }
  }
  for &extra in &rule.extra {
    let (from, to) = span(extra)?;
    if !moved.contains(&extra.as_second()) && to > window_start && from < window_end {
      out.push((from, to));
    }
  }
  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;
  use jiff::tz::TimeZone;

  fn date(y: i16, m: i8, d: i8) -> Result<Date> {
    Ok(Date::new(y, m, d)?)
  }

  fn at(d: Date, hour: i8, zone: &TimeZone) -> Result<Zoned> {
    Ok(d.at(hour, 0, 0, 0).to_zoned(zone.clone())?)
  }

  fn rule(freq: Freq) -> Rule {
    Rule {
      freq,
      interval: 1,
      ..Rule::default()
    }
  }

  #[test]
  fn weekly_on_listed_days_with_an_interval() -> Result<()> {
    // Every other week on Monday (1) and Wednesday (3), from a Monday.
    let anchor = date(2026, 1, 5)?;
    let mut r = rule(Freq::Weekly);
    r.interval = 2;
    r.by_day = vec![1, 3];
    // Weeks are produced whole, so a week straddling the stop runs past it; expansion trims by window.
    let stop = add_days(anchor, 27);
    let days: Vec<Date> = candidate_days(&r, anchor, stop)
      .into_iter()
      .filter(|d| *d <= stop)
      .collect();
    let expected: Vec<Date> = [0, 2, 14, 16]
      .iter()
      .map(|&n| add_days(anchor, n))
      .collect();
    assert_eq!(days, expected);
    Ok(())
  }

  #[test]
  fn monthly_by_ordinal_weekday_and_last_day() -> Result<()> {
    let anchor = date(2026, 1, 1)?;
    let mut second_tuesday = rule(Freq::Monthly);
    second_tuesday.by_day = vec![2];
    second_tuesday.by_day_ordinal = vec![2];
    for d in candidate_days(&second_tuesday, anchor, date(2026, 6, 30)?) {
      assert_eq!(store_weekday(d), 2);
      assert!((8..=14).contains(&d.day()), "{d} is not a second Tuesday");
    }
    let mut last_day = rule(Freq::Monthly);
    last_day.by_month_day = vec![-1];
    for d in candidate_days(&last_day, anchor, date(2026, 12, 31)?) {
      assert_eq!(d, d.last_of_month());
    }
    Ok(())
  }

  #[test]
  fn yearly_skips_a_missing_day() -> Result<()> {
    let anchor = date(2024, 2, 29)?;
    let days = candidate_days(&rule(Freq::Yearly), anchor, date(2028, 12, 31)?);
    assert!(days.iter().all(|d| d.month() == 2 && d.day() == 29));
    assert!(days.contains(&anchor));
    assert!(!days.iter().any(|d| d.year() == 2025));
    Ok(())
  }

  #[test]
  fn expansion_honours_count_exceptions_and_moves() -> Result<()> {
    let zone = TimeZone::get("Europe/London")?;
    let first = date(2026, 3, 23)?;
    let start = at(first, 9, &zone)?;
    let end = at(first, 10, &zone)?;
    let mut r = rule(Freq::Daily);
    r.count = 7;
    let skipped = at(add_days(first, 1), 9, &zone)?.timestamp().as_second();
    r.exceptions.insert(skipped);
    let moved_start = at(add_days(first, 2), 9, &zone)?.timestamp().as_second();
    let moved = BTreeSet::from([moved_start]);
    let window_start = start.timestamp();
    let window_end = at(add_days(first, 30), 0, &zone)?.timestamp();
    let found = expand(&start, &end, &r, &moved, window_start, window_end)?;
    // Seven by count, less the exception and the moved one.
    assert_eq!(found.len(), usize::try_from(r.count)? - 2);
    assert!(
      found
        .iter()
        .all(|(s, _)| s.as_second() != skipped && s.as_second() != moved_start)
    );
    Ok(())
  }

  #[test]
  fn wall_time_holds_across_a_clock_change() -> Result<()> {
    // London moves to summer time on 29 March 2026.
    let zone = TimeZone::get("Europe/London")?;
    let first = date(2026, 3, 27)?;
    let start = at(first, 9, &zone)?;
    let end = at(first, 10, &zone)?;
    let window_end = at(add_days(first, 5), 0, &zone)?.timestamp();
    let found = expand(
      &start,
      &end,
      &rule(Freq::Daily),
      &BTreeSet::new(),
      start.timestamp(),
      window_end,
    )?;
    for (s, _) in found {
      assert_eq!(s.to_zoned(zone.clone()).hour(), start.hour());
    }
    Ok(())
  }

  #[test]
  fn until_is_inclusive() -> Result<()> {
    let zone = TimeZone::UTC;
    let first = date(2026, 5, 4)?;
    let start = at(first, 9, &zone)?;
    let end = at(first, 10, &zone)?;
    let last = at(add_days(first, 3), 9, &zone)?;
    let mut r = rule(Freq::Daily);
    r.until = Some(last.timestamp());
    let window_end = at(add_days(first, 10), 0, &zone)?.timestamp();
    let found = expand(
      &start,
      &end,
      &r,
      &BTreeSet::new(),
      start.timestamp(),
      window_end,
    )?;
    assert_eq!(found.last().map(|(s, _)| *s), Some(last.timestamp()));
    Ok(())
  }
}
