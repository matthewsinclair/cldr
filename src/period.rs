//! The spans of time cldr reports on.

use jiff::ToSpan;
use jiff::civil::Date;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
  /// The Monday-to-Sunday week holding the date.
  Week(Date),
  /// The calendar month holding the date.
  Month(Date),
  /// The calendar year holding the date.
  Year(Date),
}

impl Period {
  /// First and last day, both inclusive.
  pub fn days(self) -> (Date, Date) {
    match self {
      Period::Week(d) => {
        let monday = d.saturating_sub(i64::from(d.weekday().to_monday_zero_offset()).days());
        (monday, monday.saturating_add(6.days()))
      }
      Period::Month(d) => (d.first_of_month(), d.last_of_month()),
      Period::Year(d) => (d.first_of_year(), d.last_of_year()),
    }
  }

  /// Every date, first to last.
  pub fn dates(self) -> impl Iterator<Item = Date> {
    let (first, last) = self.days();
    first.series(1.day()).take_while(move |d| *d <= last)
  }

  /// The file's place in the journal: `YYYY/YYYYMM/` of the last day (a week's Sunday), and its name.
  pub fn file(self, root: &Path) -> PathBuf {
    let (first, last) = self.days();
    let folder = root
      .join(last.strftime("%Y").to_string())
      .join(last.strftime("%Y%m").to_string());
    let span = format!("{}-{}", first.strftime("%Y%m%d"), last.strftime("%Y%m%d"));
    let name = match self {
      Period::Week(_) => format!("{span} [{:02}] Calendar.csv", first.iso_week_date().week()),
      Period::Month(_) | Period::Year(_) => format!("{span} Calendar.csv"),
    };
    folder.join(name)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Result;

  #[test]
  fn a_week_runs_monday_to_sunday_and_files_under_the_sunday() -> Result<()> {
    // A week that starts in one month and ends in the next.
    let wednesday = Date::new(2026, 10, 28)?;
    let (first, last) = Period::Week(wednesday).days();
    assert_eq!(first.weekday(), jiff::civil::Weekday::Monday);
    assert_eq!(first.saturating_add(6.days()), last);
    let path = Period::Week(wednesday).file(Path::new("/j"));
    let expected = format!(
      "/j/{}/{}/{}-{} [{:02}] Calendar.csv",
      last.strftime("%Y"),
      last.strftime("%Y%m"),
      first.strftime("%Y%m%d"),
      last.strftime("%Y%m%d"),
      first.iso_week_date().week()
    );
    assert_eq!(path, PathBuf::from(expected));
    assert_ne!(first.month(), last.month());
    Ok(())
  }

  #[test]
  fn a_year_has_every_date_once() -> Result<()> {
    let year = Period::Year(Date::new(2026, 6, 1)?);
    let (first, last) = year.days();
    let count = year.dates().count();
    assert_eq!(count, usize::try_from(last.since(first)?.get_days())? + 1);
    Ok(())
  }
}
