//! BusyCal's local store: every account BusyCal syncs, in one Core Data SQLite file.
//!
//! The schema is undocumented; what this relies on is set out in TN0001. The store is
//! opened read-only and never written.

use super::archive;
use super::recur::{self, Freq, Rule};
use super::{CalendarSource, Event, Filter, Response, When};
use crate::Result;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, Row};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

/// The store, relative to the home directory.
pub const STORE: &str =
  "Library/Group Containers/N4RA379GBW.com.busymac.busycal3/Data/BusyCal.bsql";

/// Seconds from the Unix epoch to 2001-01-01, the store's epoch.
const EPOCH_2001: f64 = 978_307_200.0;
const ENTITY_EVENT: i64 = 5;
const ENTITY_CHANGED: i64 = 6;
const ENTITY_CALENDAR: i64 = 8;
const STATUS_CANCELLED: i64 = 7;

struct CalendarInfo {
  label: String,
  hidden: bool,
}

pub struct BusyCal {
  db: Connection,
  zone: TimeZone,
  me: Vec<String>,
}

impl BusyCal {
  pub fn open(path: &Path, zone: TimeZone, me: &[String]) -> Result<BusyCal> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let db =
      Connection::open_with_flags(path, flags).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(BusyCal::from_connection(db, zone, me))
  }

  pub fn from_connection(db: Connection, zone: TimeZone, me: &[String]) -> BusyCal {
    BusyCal {
      db,
      zone,
      me: me.iter().map(|m| m.to_lowercase()).collect(),
    }
  }

  /// The number of calendars, which also proves the store can be read.
  pub fn calendar_count(&self) -> Result<usize> {
    Ok(self.calendars()?.len())
  }

  fn calendars(&self) -> Result<HashMap<i64, CalendarInfo>> {
    let mut statement = self.db.prepare(
      "select c.Z_PK, c.ZTITLE, c.ZLOCALHIDDEN, s.ZTITLE from ZCOMMONDATA c \
       left join ZSYNCABLEOBJECT so on so.Z_PK = c.ZOBJECTINFO \
       left join ZREMOTEOBJECT r on r.Z_PK = so.ZREMOTEOBJECT \
       left join ZCOMMONSERVICE s on s.Z_PK = r.ZTHEREMOTESERVICE \
       where c.Z_ENT = ?1",
    )?;
    let mut out = HashMap::new();
    let mut rows = statement.query([ENTITY_CALENDAR])?;
    while let Some(row) = rows.next()? {
      let pk = int(row, 0).unwrap_or_default();
      let name = blob(row, 1)
        .and_then(|b| archive::string(&b))
        .or_else(|| text(row, 1))
        .unwrap_or_else(|| "?".into());
      let account = text(row, 3).unwrap_or_else(|| "local".into());
      out.insert(
        pk,
        CalendarInfo {
          label: format!("{name} ({account})"),
          hidden: int(row, 2).unwrap_or(0) != 0,
        },
      );
    }
    Ok(out)
  }

  /// The user's addresses: the configured ones and any calendar named by an address.
  fn addresses(&self, calendars: &HashMap<i64, CalendarInfo>) -> BTreeSet<String> {
    let mut me: BTreeSet<String> = self.me.iter().cloned().collect();
    for info in calendars.values() {
      if let Some((name, _)) = info.label.rsplit_once(" (")
        && name.contains('@')
      {
        me.insert(name.to_lowercase());
      }
    }
    me
  }

  fn rule(&self, pk: i64) -> Result<Option<Rule>> {
    let mut statement = self.db.prepare(
      "select ZFREQUENCY, ZINTERVAL, ZRRCOUNT, ZUNTIL, ZBYDAYDAYS, ZBYDAYFREQ, ZBYMONTHDAY, ZBYMONTH, \
       ZBYSETPOS, ZWEEKSTARTDAY, ZEXCEPTIONDATES, ZCUSTOMDATES from ZRECURRENCERULEEXTRA where Z_PK = ?1",
    )?;
    let mut rows = statement.query([pk])?;
    let Some(row) = rows.next()? else {
      return Ok(None);
    };
    let Some(freq) = int(row, 0).and_then(Freq::from_store) else {
      return Ok(None);
    };
    let list = |i| {
      blob(row, i)
        .map(|b| archive::integers(&b))
        .unwrap_or_default()
    };
    let stamps = |i| -> Vec<Timestamp> {
      text(row, i)
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| stamp(s, &self.zone))
        .map(|z| z.timestamp())
        .collect()
    };
    Ok(Some(Rule {
      freq,
      interval: int(row, 1).filter(|n| *n > 0).unwrap_or(1),
      count: int(row, 2).unwrap_or(0),
      until: real(row, 3).and_then(seconds_2001),
      by_day: list(4),
      by_day_ordinal: list(5),
      by_month_day: list(6),
      by_month: list(7),
      by_set_pos: list(8),
      week_start: int(row, 9).unwrap_or(0),
      exceptions: stamps(10).iter().map(|t| t.as_second()).collect(),
      extra: stamps(11),
    }))
  }

  fn uid(&self, object: Option<i64>, pk: i64) -> Result<String> {
    if let Some(object) = object {
      let found: Option<String> = self
        .db
        .query_row(
          "select ZUID from ZSYNCABLEOBJECT where Z_PK = ?1",
          [object],
          |row| Ok(text(row, 0)),
        )
        .unwrap_or(None);
      if let Some(uid) = found.filter(|u| !u.is_empty()) {
        return Ok(uid);
      }
    }
    Ok(format!("pk{pk}"))
  }

  fn response(&self, pk: i64, me: &BTreeSet<String>) -> Result<Option<Response>> {
    let mut statement = self
      .db
      .prepare("select ZEMAIL, ZSTATUS from ZATTENDEEEXTRA where ZPARENTOBJECT = ?1")?;
    let mut rows = statement.query([pk])?;
    while let Some(row) = rows.next()? {
      let email = text(row, 0).unwrap_or_default().to_lowercase();
      if me.contains(&email) {
        return Ok(match int(row, 1) {
          Some(1) => Some(Response::NeedsAction),
          Some(2) => Some(Response::Tentative),
          Some(3) => Some(Response::Accepted),
          Some(9) => Some(Response::Declined),
          _ => None,
        });
      }
    }
    Ok(None)
  }

  /// Original starts, in whole seconds, of changed occurrences, by the event they came from.
  fn moved(&self) -> Result<HashMap<i64, BTreeSet<i64>>> {
    let mut statement = self.db.prepare(
      "select ZORIGINALEVENT, ZORIGINALTIMEANDZONE from ZCOMMONDATA where Z_ENT = ?1 and ZORIGINALEVENT is not null",
    )?;
    let mut rows = statement.query([ENTITY_CHANGED])?;
    let mut out: HashMap<i64, BTreeSet<i64>> = HashMap::new();
    while let Some(row) = rows.next()? {
      if let (Some(original), Some(when)) = (
        int(row, 0),
        text(row, 1).and_then(|t| stamp(&t, &self.zone)),
      ) {
        out
          .entry(original)
          .or_default()
          .insert(when.timestamp().as_second());
      }
    }
    Ok(out)
  }
}

struct Found {
  start: Timestamp,
  end: Timestamp,
  all_day: bool,
  zone: TimeZone,
  pk: i64,
  object: Option<i64>,
  title: String,
  location: Option<String>,
  recurring: bool,
  cancelled: bool,
  calendar: String,
}

impl CalendarSource for BusyCal {
  fn events(&self, first: Date, last: Date, filter: &Filter) -> Result<Vec<Event>> {
    let calendars = self.calendars()?;
    let me = self.addresses(&calendars);
    let moved = self.moved()?;
    let window_start = first.to_zoned(self.zone.clone())?.timestamp();
    let window_end = last
      .saturating_add(1.day())
      .to_zoned(self.zone.clone())?
      .timestamp();
    let base = Date::new(2001, 1, 1)?;
    // The store indexes events by local day number; widen by two days for zone differences.
    let day0 = i64::from(first.since(base)?.get_days()) - 2;
    let day1 = i64::from(last.since(base)?.get_days()) + 2;

    let mut statement = self.db.prepare(
      "select Z_PK, Z_ENT, ZLOCALTITLESEARCHTEXT, ZTITLE, ZSTARTTIMEANDZONE, ZENDTIMEANDZONE, ZALLDAY, \
       ZRECURRENCE, ZSTATUS, ZPARENTCONTAINERDIRECT, ZLOCATION, ZOBJECTINFO from ZCOMMONDATA \
       where Z_ENT in (?1, ?2) and coalesce(ZEVENTTYPE, 0) = 0 and ZSTARTTIMEANDZONE is not null \
       and ZLOCALFIRSTDAY <= ?3 and (ZLOCALLASTDAY >= ?4 or ZRECURRENCE is not null)",
    )?;
    let mut rows = statement.query(rusqlite::params![ENTITY_EVENT, ENTITY_CHANGED, day1, day0])?;
    let mut found = Vec::new();
    while let Some(row) = rows.next()? {
      let pk = int(row, 0).unwrap_or_default();
      let entity = int(row, 1).unwrap_or_default();
      let Some(info) = int(row, 9).and_then(|c| calendars.get(&c)) else {
        continue;
      };
      if info.hidden || !filter.admits_calendar(&info.label) {
        continue;
      }
      let Some(start) = text(row, 4).and_then(|t| stamp(&t, &self.zone)) else {
        continue;
      };
      let end = text(row, 5)
        .and_then(|t| stamp(&t, &self.zone))
        .unwrap_or_else(|| start.clone());
      let all_day = int(row, 6).unwrap_or(0) != 0;
      let recurrence = int(row, 7);
      let title = blob(row, 3)
        .and_then(|b| archive::string(&b))
        .or_else(|| {
          text(row, 2)
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
        })
        .unwrap_or_else(|| "(no title)".into());
      let spans = match (recurrence, entity) {
        (Some(rule_pk), ENTITY_EVENT) => match self.rule(rule_pk)? {
          Some(rule) => {
            let none = BTreeSet::new();
            recur::expand(
              &start,
              &end,
              &rule,
              moved.get(&pk).unwrap_or(&none),
              window_start,
              window_end,
            )?
          }
          None => Vec::new(),
        },
        _ if all_day => {
          let (from, to) = (start.date(), end.date());
          if to > first && from <= last {
            vec![(start.timestamp(), end.timestamp())]
          } else {
            Vec::new()
          }
        }
        _ => {
          let (from, to) = (start.timestamp(), end.timestamp());
          if to > window_start && from < window_end {
            vec![(from, to)]
          } else {
            Vec::new()
          }
        }
      };
      for (from, to) in spans {
        found.push(Found {
          start: from,
          end: to,
          all_day,
          zone: start.time_zone().clone(),
          pk,
          object: int(row, 11),
          title: title.clone(),
          location: text(row, 10),
          recurring: recurrence.is_some() || entity == ENTITY_CHANGED,
          cancelled: int(row, 8) == Some(STATUS_CANCELLED),
          calendar: info.label.clone(),
        });
      }
    }
    drop(rows);

    // The same invitation on two calendars shares a UID: report it once, naming both.
    let mut merged: BTreeMap<(String, i64), Event> = BTreeMap::new();
    for f in found {
      let uid = self.uid(f.object, f.pk)?;
      let response = self.response(f.pk, &me)?;
      let event = Event {
        uid: uid.clone(),
        title: f.title,
        when: if f.all_day {
          let first_day = f.start.to_zoned(f.zone.clone()).date();
          let end_day = f.end.to_zoned(f.zone).date();
          // All-day ends are exclusive in the store.
          When::AllDay {
            first: first_day,
            last: end_day.saturating_sub(1.day()).max(first_day),
          }
        } else {
          When::Timed {
            start: f.start.to_zoned(self.zone.clone()),
            end: f.end.to_zoned(self.zone.clone()),
          }
        },
        calendars: vec![f.calendar.clone()],
        location: f.location,
        recurring: f.recurring,
        response,
        cancelled: f.cancelled,
      };
      if event.dropped() && !filter.dropped {
        continue;
      }
      match merged.get_mut(&(uid.clone(), f.start.as_second())) {
        Some(existing) => {
          if !existing.calendars.contains(&f.calendar) {
            existing.calendars.push(f.calendar);
          }
        }
        None => {
          merged.insert((uid, f.start.as_second()), event);
        }
      }
    }
    let mut events: Vec<Event> = merged.into_values().collect();
    events.sort_by_key(|e| {
      let instant = match &e.when {
        When::Timed { start, .. } => start.timestamp().as_second(),
        When::AllDay { .. } => i64::MIN,
      };
      (e.when.first_date(), instant, e.title.clone())
    });
    Ok(events)
  }
}

/// "811328400:Europe/London" as a zoned time in its own zone. Floating and unnamed times read as UTC.
fn stamp(value: &str, fallback: &TimeZone) -> Option<Zoned> {
  let (seconds, zone) = value.split_once(':').unwrap_or((value, ""));
  let seconds: f64 = seconds.trim().parse().ok()?;
  let zone = match zone {
    "" | "Universal" | "floating" => TimeZone::UTC,
    name => TimeZone::get(name).unwrap_or_else(|_| fallback.clone()),
  };
  Some(seconds_2001(seconds)?.to_zoned(zone))
}

fn seconds_2001(seconds: f64) -> Option<Timestamp> {
  let micros = ((EPOCH_2001 + seconds) * 1_000_000.0).round();
  if !micros.is_finite() {
    return None;
  }
  // Rounded to whole seconds: occurrence keys compare by the second.
  Timestamp::from_second((micros / 1_000_000.0).round() as i64).ok()
}

fn value(row: &Row, index: usize) -> Value {
  row.get::<_, Value>(index).unwrap_or(Value::Null)
}

fn int(row: &Row, index: usize) -> Option<i64> {
  match value(row, index) {
    Value::Integer(n) => Some(n),
    Value::Real(f) if f.is_finite() => Some(f as i64),
    _ => None,
  }
}

fn real(row: &Row, index: usize) -> Option<f64> {
  match value(row, index) {
    Value::Integer(n) => Some(n as f64),
    Value::Real(f) => Some(f),
    _ => None,
  }
}

fn text(row: &Row, index: usize) -> Option<String> {
  match value(row, index) {
    Value::Text(t) => Some(t),
    _ => None,
  }
}

fn blob(row: &Row, index: usize) -> Option<Vec<u8>> {
  match value(row, index) {
    Value::Blob(b) => Some(b),
    _ => None,
  }
}

#[cfg(test)]
pub mod fixture {
  //! A synthetic store in the same shape, built by the tests.

  use super::*;
  use crate::calendar::archive::build;

  pub struct Store {
    pub db: Connection,
    next: i64,
  }

  pub fn seconds(at: &Zoned) -> f64 {
    at.timestamp().as_second() as f64 - EPOCH_2001
  }

  pub fn stamp_text(at: &Zoned) -> String {
    format!(
      "{}:{}",
      seconds(at),
      at.time_zone().iana_name().unwrap_or("UTC")
    )
  }

  fn local_day(date: Date) -> Result<i64> {
    Ok(i64::from(date.since(Date::new(2001, 1, 1)?)?.get_days()))
  }

  pub struct Spec<'a> {
    pub calendar: i64,
    pub title: &'a str,
    pub start: Zoned,
    pub end: Zoned,
    pub all_day: bool,
    pub uid: &'a str,
    pub status: Option<i64>,
    pub rule: Option<i64>,
    pub original: Option<(i64, Zoned)>,
  }

  impl Store {
    pub fn new() -> Result<Store> {
      let db = Connection::open_in_memory()?;
      db.execute_batch(
        "create table ZCOMMONDATA (Z_PK integer primary key, Z_ENT integer, ZTITLE blob, ZLOCALHIDDEN integer, \
           ZOBJECTINFO integer, ZLOCALTITLESEARCHTEXT text, ZSTARTTIMEANDZONE text, ZENDTIMEANDZONE text, \
           ZALLDAY integer, ZRECURRENCE integer, ZSTATUS integer, ZPARENTCONTAINERDIRECT integer, \
           ZORIGINALEVENT integer, ZORIGINALTIMEANDZONE text, ZLOCATION text, ZEVENTTYPE integer, \
           ZLOCALFIRSTDAY integer, ZLOCALLASTDAY integer);
         create table ZSYNCABLEOBJECT (Z_PK integer primary key, ZREMOTEOBJECT integer, ZUID text);
         create table ZREMOTEOBJECT (Z_PK integer primary key, ZTHEREMOTESERVICE integer);
         create table ZCOMMONSERVICE (Z_PK integer primary key, ZTITLE text);
         create table ZATTENDEEEXTRA (ZPARENTOBJECT integer, ZNAME text, ZEMAIL text, ZSTATUS integer, ZROLE integer);
         create table ZRECURRENCERULEEXTRA (Z_PK integer primary key, ZFREQUENCY integer, ZINTERVAL integer, \
           ZRRCOUNT integer, ZUNTIL real, ZBYDAYDAYS blob, ZBYDAYFREQ blob, ZBYMONTHDAY blob, ZBYMONTH blob, \
           ZBYSETPOS blob, ZWEEKSTARTDAY integer, ZEXCEPTIONDATES text, ZCUSTOMDATES text);",
      )?;
      Ok(Store { db, next: 1000 })
    }

    fn id(&mut self) -> i64 {
      self.next += 1;
      self.next
    }

    pub fn calendar(&mut self, name: &str, account: &str, hidden: bool) -> Result<i64> {
      let (service, remote, object, pk) = (self.id(), self.id(), self.id(), self.id());
      self.db.execute(
        "insert into ZCOMMONSERVICE values (?1, ?2)",
        rusqlite::params![service, account],
      )?;
      self.db.execute(
        "insert into ZREMOTEOBJECT values (?1, ?2)",
        rusqlite::params![remote, service],
      )?;
      self.db.execute(
        "insert into ZSYNCABLEOBJECT values (?1, ?2, null)",
        rusqlite::params![object, remote],
      )?;
      self.db.execute(
        "insert into ZCOMMONDATA (Z_PK, Z_ENT, ZTITLE, ZLOCALHIDDEN, ZOBJECTINFO) values (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![pk, ENTITY_CALENDAR, build::string(name), i64::from(hidden), object],
      )?;
      Ok(pk)
    }

    pub fn event(&mut self, spec: Spec) -> Result<i64> {
      let (object, pk) = (self.id(), self.id());
      self.db.execute(
        "insert into ZSYNCABLEOBJECT values (?1, null, ?2)",
        rusqlite::params![object, spec.uid],
      )?;
      let entity = if spec.original.is_some() {
        ENTITY_CHANGED
      } else {
        ENTITY_EVENT
      };
      let (original, original_at) = match &spec.original {
        Some((pk, at)) => (Some(*pk), Some(stamp_text(at))),
        None => (None, None),
      };
      let last = if spec.rule.is_some() {
        local_day(spec.start.date())?
      } else {
        local_day(spec.end.date())?
      };
      self.db.execute(
        "insert into ZCOMMONDATA (Z_PK, Z_ENT, ZTITLE, ZOBJECTINFO, ZSTARTTIMEANDZONE, ZENDTIMEANDZONE, ZALLDAY, \
           ZRECURRENCE, ZSTATUS, ZPARENTCONTAINERDIRECT, ZORIGINALEVENT, ZORIGINALTIMEANDZONE, ZLOCALFIRSTDAY, \
           ZLOCALLASTDAY) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
          pk,
          entity,
          build::string(spec.title),
          object,
          stamp_text(&spec.start),
          stamp_text(&spec.end),
          i64::from(spec.all_day),
          spec.rule,
          spec.status,
          spec.calendar,
          original,
          original_at,
          local_day(spec.start.date())?,
          last
        ],
      )?;
      Ok(pk)
    }

    pub fn weekly(&mut self, days: &[i64], count: i64, exceptions: &[Zoned]) -> Result<i64> {
      let pk = self.id();
      let exceptions: Vec<String> = exceptions.iter().map(stamp_text).collect();
      self.db.execute(
        "insert into ZRECURRENCERULEEXTRA (Z_PK, ZFREQUENCY, ZINTERVAL, ZRRCOUNT, ZBYDAYDAYS, ZWEEKSTARTDAY, \
           ZEXCEPTIONDATES) values (?1, 1, 1, ?2, ?3, 0, ?4)",
        rusqlite::params![pk, count, build::integers(days), exceptions.join(",")],
      )?;
      Ok(pk)
    }

    pub fn attendee(&mut self, event: i64, email: &str, status: i64) -> Result<()> {
      self.db.execute(
        "insert into ZATTENDEEEXTRA values (?1, null, ?2, ?3, 0)",
        rusqlite::params![event, email, status],
      )?;
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::fixture::{Spec, Store};
  use super::*;

  const ZONE: &str = "Europe/London";

  fn zone() -> Result<TimeZone> {
    Ok(TimeZone::get(ZONE)?)
  }

  fn at(date: Date, hour: i8) -> Result<Zoned> {
    Ok(date.at(hour, 0, 0, 0).to_zoned(zone()?)?)
  }

  fn spec<'a>(calendar: i64, title: &'a str, uid: &'a str, start: Zoned, end: Zoned) -> Spec<'a> {
    Spec {
      calendar,
      title,
      start,
      end,
      all_day: false,
      uid,
      status: None,
      rule: None,
      original: None,
    }
  }

  fn read(store: Store, first: Date, last: Date, filter: &Filter) -> Result<Vec<Event>> {
    let me = vec!["me@example.com".to_owned()];
    BusyCal::from_connection(store.db, zone()?, &me).events(first, last, filter)
  }

  #[test]
  fn a_timed_event_reads_in_the_journal_zone() -> Result<()> {
    let mut store = Store::new()?;
    let cal = store.calendar("Work", "me@example.com", false)?;
    let day = Date::new(2026, 10, 19)?;
    let title = "Planning";
    store.event(spec(cal, title, "a", at(day, 10)?, at(day, 11)?))?;
    let events = read(store, day, day, &Filter::default())?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].title, title);
    match &events[0].when {
      When::Timed { start, .. } => assert_eq!(start.hour(), 10),
      other => return Err(format!("expected a timed event, got {other:?}").into()),
    }
    Ok(())
  }

  #[test]
  fn all_day_ends_are_exclusive() -> Result<()> {
    let mut store = Store::new()?;
    let cal = store.calendar("Home", "me@example.com", false)?;
    let first = Date::new(2026, 10, 14)?;
    let after = Date::new(2026, 10, 17)?;
    let noon = |d: Date| -> Result<Zoned> { Ok(d.at(12, 0, 0, 0).to_zoned(TimeZone::UTC)?) };
    let mut s = spec(cal, "Away", "b", noon(first)?, noon(after)?);
    s.all_day = true;
    store.event(s)?;
    let events = read(store, first, after, &Filter::default())?;
    assert_eq!(events.len(), 1);
    assert_eq!(
      events[0].when,
      When::AllDay {
        first,
        last: after.saturating_sub(1.day())
      }
    );
    Ok(())
  }

  #[test]
  fn a_weekly_series_skips_its_exception_and_takes_its_moved_copy() -> Result<()> {
    let mut store = Store::new()?;
    let cal = store.calendar("Work", "me@example.com", false)?;
    let monday = Date::new(2026, 10, 5)?;
    let weeks = 4;
    let skipped = at(monday.saturating_add(7.days()), 9)?;
    let moved_from = at(monday.saturating_add(14.days()), 9)?;
    let rule = store.weekly(&[1], weeks, std::slice::from_ref(&skipped))?;
    let mut series = spec(cal, "Stand-up", "s", at(monday, 9)?, at(monday, 10)?);
    series.rule = Some(rule);
    let master = store.event(series)?;
    let moved_to = moved_from.date().saturating_add(1.day());
    let mut changed = spec(
      cal,
      "Stand-up (moved)",
      "s",
      at(moved_to, 15)?,
      at(moved_to, 16)?,
    );
    changed.original = Some((master, moved_from.clone()));
    store.event(changed)?;
    let last = monday.saturating_add((7 * weeks).days());
    let events = read(store, monday, last, &Filter::default())?;
    let starts: Vec<Date> = events.iter().map(|e| e.when.first_date()).collect();
    assert!(!starts.contains(&skipped.date()));
    assert!(!starts.contains(&moved_from.date()));
    assert!(starts.contains(&moved_to));
    // Four by count, less the exception, the moved one replaced by its copy.
    assert_eq!(events.len(), usize::try_from(weeks)? - 1);
    Ok(())
  }

  #[test]
  fn declined_cancelled_and_hidden_are_left_out() -> Result<()> {
    let mut store = Store::new()?;
    let cal = store.calendar("Work", "me@example.com", false)?;
    let hidden = store.calendar("Old", "me@example.com", true)?;
    let day = Date::new(2026, 10, 20)?;
    let declined = store.event(spec(cal, "Declined", "d", at(day, 9)?, at(day, 10)?))?;
    store.attendee(declined, "ME@example.com", 9)?;
    let mut cancelled = spec(cal, "Cancelled", "c", at(day, 11)?, at(day, 12)?);
    cancelled.status = Some(STATUS_CANCELLED);
    store.event(cancelled)?;
    store.event(spec(hidden, "Hidden", "h", at(day, 13)?, at(day, 14)?))?;
    store.event(spec(cal, "Kept", "k", at(day, 15)?, at(day, 16)?))?;
    let kept: Vec<String> = read(store, day, day, &Filter::default())?
      .into_iter()
      .map(|e| e.title)
      .collect();
    assert_eq!(kept, vec!["Kept".to_owned()]);
    Ok(())
  }

  #[test]
  fn dropped_events_come_back_when_asked_for() -> Result<()> {
    let mut store = Store::new()?;
    let cal = store.calendar("Work", "me@example.com", false)?;
    let day = Date::new(2026, 10, 20)?;
    let declined = store.event(spec(cal, "Declined", "d", at(day, 9)?, at(day, 10)?))?;
    store.attendee(declined, "me@example.com", 9)?;
    let filter = Filter {
      dropped: true,
      ..Filter::default()
    };
    let events = read(store, day, day, &filter)?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].response, Some(Response::Declined));
    Ok(())
  }

  #[test]
  fn one_invitation_on_two_calendars_is_one_event() -> Result<()> {
    let mut store = Store::new()?;
    let work = store.calendar("Work", "me@work.example", false)?;
    let home = store.calendar("Home", "me@example.com", false)?;
    let day = Date::new(2026, 10, 21)?;
    store.event(spec(work, "Review", "same", at(day, 14)?, at(day, 15)?))?;
    store.event(spec(home, "Review", "same", at(day, 14)?, at(day, 15)?))?;
    let events = read(store, day, day, &Filter::default())?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].calendars.len(), 2);
    Ok(())
  }

  #[test]
  fn the_filter_selects_by_calendar_or_account() -> Result<()> {
    let mut store = Store::new()?;
    let work = store.calendar("Work", "me@work.example", false)?;
    let home = store.calendar("Home", "me@example.com", false)?;
    let day = Date::new(2026, 10, 22)?;
    store.event(spec(work, "Work thing", "w", at(day, 9)?, at(day, 10)?))?;
    store.event(spec(home, "Home thing", "h", at(day, 11)?, at(day, 12)?))?;
    let filter = Filter {
      include: vec!["work.example".into()],
      ..Filter::default()
    };
    let titles: Vec<String> = read(store, day, day, &filter)?
      .into_iter()
      .map(|e| e.title)
      .collect();
    assert_eq!(titles, vec!["Work thing".to_owned()]);
    Ok(())
  }
}
