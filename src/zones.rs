//! Dated zones (D3): the journal's wall clock follows where the user was.
//!
//! The config gives a default zone and a list of changes, each from a date on. A change takes
//! effect at local midnight of its date in the new zone, so every date and every instant has
//! exactly one zone.

use crate::Result;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

#[derive(Debug, Clone)]
struct Change {
  from: Date,
  starts: Timestamp,
  zone: TimeZone,
}

#[derive(Debug, Clone)]
pub struct Zones {
  default: TimeZone,
  /// Ordered by date, one per date.
  changes: Vec<Change>,
}

impl Zones {
  /// One zone for every date.
  pub fn fixed(zone: TimeZone) -> Zones {
    Zones {
      default: zone,
      changes: Vec::new(),
    }
  }

  /// The default zone and the dated changes, in any order. Two changes on one date are refused.
  pub fn new(default: TimeZone, changes: Vec<(Date, TimeZone)>) -> Result<Zones> {
    let mut list = Vec::with_capacity(changes.len());
    for (from, zone) in changes {
      let starts = from.to_zoned(zone.clone())?.timestamp();
      list.push(Change { from, starts, zone });
    }
    list.sort_by_key(|c| c.from);
    if let Some(pair) = list.windows(2).find(|w| w[0].from == w[1].from) {
      return Err(format!("journal.zones: two zones from {}", pair[0].from).into());
    }
    Ok(Zones {
      default,
      changes: list,
    })
  }

  /// The zone the journal's day `date` is kept in.
  pub fn on(&self, date: Date) -> &TimeZone {
    self
      .changes
      .iter()
      .rev()
      .find(|c| c.from <= date)
      .map_or(&self.default, |c| &c.zone)
  }

  /// The zone in force at an instant.
  pub fn at(&self, instant: Timestamp) -> &TimeZone {
    self
      .changes
      .iter()
      .rev()
      .find(|c| c.starts <= instant)
      .map_or(&self.default, |c| &c.zone)
  }

  /// The dated changes, in date order.
  pub fn changes(&self) -> Vec<(Date, &TimeZone)> {
    self.changes.iter().map(|c| (c.from, &c.zone)).collect()
  }

  /// The zone to fall back on where nothing else says.
  pub fn default_zone(&self) -> &TimeZone {
    &self.default
  }

  /// The instant a journal day starts: midnight in its zone, or midnight in the previous
  /// day's zone when that comes first (coming back from a zone that is ahead).
  pub fn start_of(&self, date: Date) -> Result<Timestamp> {
    let own = date.to_zoned(self.on(date).clone())?.timestamp();
    let previous = date
      .to_zoned(self.on(date.saturating_sub(1.day())).clone())?
      .timestamp();
    Ok(own.min(previous))
  }

  /// The journal date of an instant.
  pub fn date_of(&self, instant: Timestamp) -> Date {
    instant.to_zoned(self.at(instant).clone()).date()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn zones() -> Result<(Zones, Date, TimeZone, TimeZone)> {
    let home = TimeZone::get("Europe/London")?;
    let away = TimeZone::get("Pacific/Auckland")?;
    let moved = Date::new(2026, 9, 27)?;
    let back = moved.saturating_add(60.days());
    let zones = Zones::new(
      home.clone(),
      vec![(back, home.clone()), (moved, away.clone())],
    )?;
    Ok((zones, moved, home, away))
  }

  #[test]
  fn each_date_takes_the_latest_change_on_or_before_it() -> Result<()> {
    let (zones, moved, home, away) = zones()?;
    assert_eq!(zones.on(moved.saturating_sub(1.day())), &home);
    assert_eq!(zones.on(moved), &away);
    assert_eq!(zones.on(moved.saturating_add(59.days())), &away);
    assert_eq!(zones.on(moved.saturating_add(60.days())), &home);
    Ok(())
  }

  #[test]
  fn a_change_starts_at_local_midnight_in_the_new_zone() -> Result<()> {
    let (zones, moved, _, away) = zones()?;
    let start = zones.start_of(moved)?;
    assert_eq!(start, moved.to_zoned(away)?.timestamp());
    assert_eq!(zones.date_of(start), moved);
    let just_before = start - 1.second();
    assert_eq!(zones.date_of(just_before), moved.saturating_sub(1.day()));
    Ok(())
  }

  #[test]
  fn coming_home_the_day_starts_at_the_earlier_midnight() -> Result<()> {
    let (zones, moved, home, away) = zones()?;
    let back = moved.saturating_add(60.days());
    let start = zones.start_of(back)?;
    assert_eq!(start, back.to_zoned(away)?.timestamp());
    assert_eq!(zones.date_of(start), back);
    assert_eq!(
      zones.date_of(start - 1.second()),
      back.saturating_sub(1.day())
    );
    let home_midnight = back.to_zoned(home)?.timestamp();
    assert_eq!(zones.date_of(home_midnight - 1.second()), back);
    Ok(())
  }

  #[test]
  fn two_changes_on_one_date_are_refused() -> Result<()> {
    let utc = TimeZone::UTC;
    let day = Date::new(2026, 1, 1)?;
    assert!(Zones::new(utc.clone(), vec![(day, utc.clone()), (day, utc)]).is_err());
    Ok(())
  }
}
