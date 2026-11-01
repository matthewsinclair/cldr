//! The calendar as CSV, in the format the old journal tooling reads:
//! `Date,From,To,Title,Modified`, dates as DD/MM/YYYY, times as HH:MM.

use crate::calendar::{Event, When};
pub use crate::period::Period;

pub const HEADER: &str = "Date,From,To,Title,Modified";

/// Quote a field only when CSV needs it.
fn field(value: &str) -> String {
  if value.contains([',', '"', '\n', '\r']) {
    format!("\"{}\"", value.replace('"', "\"\""))
  } else {
    value.to_owned()
  }
}

pub fn row(event: &Event) -> String {
  let date = event.when.first_date().strftime("%d/%m/%Y").to_string();
  let (from, to) = match &event.when {
    When::Timed { start, end } => (
      start.strftime("%H:%M").to_string(),
      end.strftime("%H:%M").to_string(),
    ),
    When::AllDay { .. } => (String::new(), String::new()),
  };
  [
    date,
    from,
    to,
    field(&one_line(&event.title)),
    String::new(),
  ]
  .join(",")
}

/// A title on one line: its lines joined by a space, control characters removed.
pub fn one_line(title: &str) -> String {
  let lines: Vec<&str> = title
    .lines()
    .map(str::trim)
    .filter(|l| !l.is_empty())
    .collect();
  crate::clean(&lines.join(" "))
}

pub fn csv(events: &[Event]) -> String {
  let mut out = String::from(HEADER);
  out.push('\n');
  for event in events {
    out.push_str(&row(event));
    out.push('\n');
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Result;
  use jiff::civil::Date;
  use jiff::tz::TimeZone;

  fn event(title: &str, when: When) -> Event {
    Event {
      uid: "u".into(),
      title: title.into(),
      when,
      calendars: vec![],
      location: None,
      recurring: false,
      response: None,
      cancelled: false,
    }
  }

  #[test]
  fn rows_quote_only_when_needed() -> Result<()> {
    let zone = TimeZone::UTC;
    let day = Date::new(2026, 10, 20)?;
    let start = day.at(8, 30, 0, 0).to_zoned(zone.clone())?;
    let end = day.at(10, 45, 0, 0).to_zoned(zone)?;
    let timed = event("Prep; deck for A, B", When::Timed { start, end });
    assert_eq!(
      row(&timed),
      "20/10/2026,08:30,10:45,\"Prep; deck for A, B\","
    );
    let all_day = event(
      "Away ",
      When::AllDay {
        first: day,
        last: day,
      },
    );
    assert_eq!(row(&all_day), "20/10/2026,,,Away,");
    Ok(())
  }
}

#[cfg(test)]
mod one_line_tests {
  use super::one_line;

  #[test]
  fn a_title_over_several_lines_becomes_one() {
    assert_eq!(
      one_line("Exam\nScheduled: 22 May \n\n"),
      "Exam Scheduled: 22 May"
    );
    assert_eq!(one_line("Tab\there"), "Tabhere");
  }
}
