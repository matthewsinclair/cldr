//! Calendar events, from whichever source the configuration names.

pub mod archive;
pub mod busycal;
pub mod recur;

use crate::Result;
use jiff::Zoned;
use jiff::civil::Date;
use serde::Serialize;

/// When an event happens: a span of wall-clock time, or whole days.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum When {
  /// Start and end in the journal's zone.
  Timed { start: Zoned, end: Zoned },
  /// Whole days, both inclusive.
  AllDay { first: Date, last: Date },
}

impl When {
  pub fn first_date(&self) -> Date {
    match self {
      When::Timed { start, .. } => start.date(),
      When::AllDay { first, .. } => *first,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Response {
  NeedsAction,
  Tentative,
  Accepted,
  Declined,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
  pub uid: String,
  pub title: String,
  pub when: When,
  /// Every calendar the event appears on, as "calendar (account)".
  pub calendars: Vec<String>,
  pub location: Option<String>,
  pub recurring: bool,
  pub response: Option<Response>,
  pub cancelled: bool,
}

impl Event {
  /// Declined or cancelled: not part of the day unless asked for.
  pub fn dropped(&self) -> bool {
    self.cancelled || self.response == Some(Response::Declined)
  }
}

/// Which events a read returns.
#[derive(Debug, Default, Clone)]
pub struct Filter {
  /// Substrings of "calendar (account)"; empty means every calendar.
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub dropped: bool,
}

impl Filter {
  pub fn admits_calendar(&self, name: &str) -> bool {
    let name = name.to_lowercase();
    let hit = |needles: &[String]| needles.iter().any(|n| name.contains(&n.to_lowercase()));
    (self.include.is_empty() || hit(&self.include)) && !hit(&self.exclude)
  }
}

pub trait CalendarSource {
  /// Events touching the days `first..=last`, in time order, all-day events first on each day.
  fn events(&self, first: Date, last: Date, filter: &Filter) -> Result<Vec<Event>>;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_filter_matches_calendar_or_account_without_case() {
    let filter = Filter {
      include: vec!["Work".into()],
      exclude: vec!["holidays".into()],
      dropped: false,
    };
    assert!(filter.admits_calendar("Calendar (you@work.example)"));
    assert!(!filter.admits_calendar("Home (you@example.com)"));
    assert!(!filter.admits_calendar("Work Holidays (you@work.example)"));
    assert!(Filter::default().admits_calendar("anything"));
  }
}
