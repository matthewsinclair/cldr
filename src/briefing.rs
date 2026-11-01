//! The Daily Briefing: one file a month, `YYYYMMDD-YYYYMMDD-daily-briefing.md`, read top down:
//!
//! - `## Briefing: <day>`: Today, Yesterday, Tomorrow
//! - `## Schedule`: This Week, This Month
//! - a section per configured block (eg the open engagements)
//! - `## Earlier`: past days' Briefing sections, newest first
//!
//! cldr writes only between `<!-- gen:NAME -->` and `<!-- /gen:NAME -->`: the calendar lines and
//! the configured blocks' output. Everything else is the user's. Briefing a new day moves the
//! previous Briefing section under Earlier, one heading level down; briefing the same day again
//! rewrites only the markers.

use crate::Result;
use crate::calendar::{Event, When};
use crate::config::BriefingBlock;
use crate::period::Period;
use jiff::ToSpan;
use jiff::civil::Date;
use std::collections::BTreeSet;

pub const DAILY_BRIEFING: &str = "daily-briefing";

fn label(day: Date) -> String {
  day.strftime("%a %-d %b").to_string()
}

fn short(day: Date) -> String {
  day.strftime("%a %-d").to_string()
}

fn gen_block(name: &str, body: &str) -> String {
  format!("<!-- gen:{name} -->\n{body}\n<!-- /gen:{name} -->")
}

/// A title for Markdown: one line, brackets escaped so it cannot become a link.
fn text(title: &str) -> String {
  crate::export::one_line(title)
    .replace('[', "\\[")
    .replace(']', "\\]")
}

fn linked(e: &Event) -> String {
  match &e.link {
    Some(link) => format!("[{}]({link})", text(&e.title)),
    None => text(&e.title),
  }
}

fn on_day(e: &Event, day: Date) -> bool {
  match &e.when {
    When::AllDay { first, last } => *first <= day && day <= *last,
    When::Timed { start, .. } => start.date() == day,
  }
}

/// The events in briefing order, each once: by day, all-day events first, then by start; ties
/// by end and title, so the order never depends on the store.
pub fn prepare(events: Vec<Event>) -> Vec<Event> {
  let mut events = dedupe(events);
  events.sort_by_key(|e| match &e.when {
    When::AllDay { first, last } => (*first, 0, String::new(), last.to_string(), e.title.clone()),
    When::Timed { start, end } => (
      start.date(),
      1,
      start.strftime("%H:%M").to_string(),
      end.to_string(),
      e.title.clone(),
    ),
  });
  events
}

/// The same event on several calendars, or entered twice, shows once.
fn dedupe(events: Vec<Event>) -> Vec<Event> {
  let mut seen = BTreeSet::new();
  events
    .into_iter()
    .filter(|e| {
      let when = match &e.when {
        When::Timed { start, end } => format!("{start}-{end}"),
        When::AllDay { first, last } => format!("{first}-{last}"),
      };
      seen.insert((when, e.title.trim().to_lowercase()))
    })
    .collect()
}

pub fn day_lines(events: &[Event], day: Date) -> String {
  let lines: Vec<String> = events
    .iter()
    .filter(|e| on_day(e, day))
    .map(|e| match &e.when {
      When::AllDay { .. } => format!("- All day: {}", linked(e)),
      When::Timed { start, end } => {
        let join = e
          .join
          .as_ref()
          .map(|j| format!(" · [join]({j})"))
          .unwrap_or_default();
        format!(
          "- {}-{} {}{join}",
          start.strftime("%H:%M"),
          end.strftime("%H:%M"),
          linked(e)
        )
      }
    })
    .collect();
  if lines.is_empty() {
    "- Nothing in the calendar.".into()
  } else {
    lines.join("\n")
  }
}

pub fn week_lines(events: &[Event], monday: Date) -> String {
  monday
    .series(1.day())
    .take(7)
    .map(|day| {
      let items: Vec<String> = events
        .iter()
        .filter(|e| on_day(e, day))
        .map(|e| match &e.when {
          When::AllDay { .. } => format!("all day {}", text(&e.title)),
          When::Timed { start, .. } => format!("{} {}", start.strftime("%H:%M"), text(&e.title)),
        })
        .collect();
      let what = if items.is_empty() {
        "nothing".to_owned()
      } else {
        items.join(" · ")
      };
      format!("- **{}**: {what}", short(day))
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// All-day events that start in the month and have not ended by `today`.
pub fn month_lines(events: &[Event], today: Date, month: Date) -> String {
  let (first, last) = Period::Month(month).days();
  let lines: Vec<String> = events
    .iter()
    .filter_map(|e| match &e.when {
      When::AllDay { first: a, last: b } if *a >= first && *a <= last && *b >= today => {
        let when = if a == b {
          short(*a)
        } else {
          format!("{} to {}", short(*a), short(*b))
        };
        Some(format!("- {when}: {}", linked(e)))
      }
      _ => None,
    })
    .collect();
  if lines.is_empty() {
    "- No all-day events left this month.".into()
  } else {
    lines.join("\n")
  }
}

fn briefing_section(day: Date) -> String {
  let (before, after) = (day.saturating_sub(1.day()), day.saturating_add(1.day()));
  format!(
    "## Briefing: {}\n\n### Today ({})\n\n{}\n\n### Yesterday ({})\n\n- \n\n### Tomorrow ({})\n\n{}\n",
    label(day),
    short(day),
    gen_block("today", ""),
    short(before),
    short(after),
    gen_block("tomorrow", "")
  )
}

/// A new month's briefing file.
pub fn skeleton(month: Date, day: Date, author: &str, blocks: &[BriefingBlock]) -> String {
  let (first, last) = Period::Month(month).days();
  let mut out = format!(
    "---\nverblock: \"{}:v0.1: {author} - Initial version\"\n---\n# {}-{} Daily Briefing\n\n{}\n\
     ## Schedule\n\n### This Week\n\n{}\n\n### This Month\n\n{}\n\n",
    first.strftime("%Y%m%d"),
    first.strftime("%Y%m%d"),
    last.strftime("%Y%m%d"),
    briefing_section(day),
    gen_block("week", ""),
    gen_block("month", "")
  );
  for block in blocks {
    out.push_str(&format!(
      "## {}\n\n{}\n\n",
      block.heading,
      gen_block(&block.name, "")
    ));
  }
  out.push_str("## Earlier\n");
  out
}

/// Move headings one level down (`##` to `###` and so on).
fn demote(section: &str) -> String {
  section
    .split_inclusive('\n')
    .map(|line| {
      let hashes = line.chars().take_while(|c| *c == '#').count();
      if (2..=5).contains(&hashes) && line[hashes..].starts_with(' ') {
        format!("#{line}")
      } else {
        line.to_owned()
      }
    })
    .collect()
}

/// Strip gen markers, keeping what was between them.
fn unmark(section: &str) -> String {
  section
    .split_inclusive('\n')
    .filter(|l| {
      let t = l.trim();
      let marker =
        (t.starts_with("<!-- gen:") || t.starts_with("<!-- /gen:")) && t.ends_with("-->");
      !marker
    })
    .collect()
}

/// Move the current Briefing under Earlier when it is for another day.
pub fn roll(doc: &str, day: Date) -> Result<String> {
  let Some(at) = doc.find("## Briefing: ") else {
    return Ok(doc.to_owned());
  };
  let line_end = doc[at..].find('\n').map_or(doc.len(), |i| at + i);
  if doc[at + "## Briefing: ".len()..line_end].trim() == label(day) {
    return Ok(doc.to_owned());
  }
  let end = at
    + doc[at..]
      .find("\n## Schedule")
      .ok_or("the briefing has no ## Schedule section")?;
  let old = format!("{}\n", doc[at..end].trim_end());
  let old = demote(&unmark(&old));
  let doc = format!("{}{}{}", &doc[..at], briefing_section(day), &doc[end..]);
  let marker = "\n## Earlier\n";
  let earlier = doc
    .find(marker)
    .ok_or("the briefing has no ## Earlier section")?
    + marker.len();
  Ok(format!("{}\n{old}{}", &doc[..earlier], &doc[earlier..]))
}

/// Replace one gen block's body.
pub fn fill(doc: &str, name: &str, body: &str) -> Result<String> {
  let start = format!("<!-- gen:{name} -->");
  let end = format!("<!-- /gen:{name} -->");
  let a = doc
    .find(&start)
    .ok_or_else(|| format!("the briefing has no gen:{name} block"))?;
  let b = a
    + doc[a..]
      .find(&end)
      .ok_or_else(|| format!("gen:{name} is not closed"))?
    + end.len();
  Ok(format!(
    "{}{}{}",
    &doc[..a],
    gen_block(name, body),
    &doc[b..]
  ))
}

/// The briefing for `day`, from the current file (or a new one), the day's events and the
/// configured blocks' output.
pub fn build(
  current: Option<&str>,
  day: Date,
  author: &str,
  events: &[Event],
  blocks: &[(BriefingBlock, String)],
) -> Result<String> {
  let month = day.first_of_month();
  let configured: Vec<BriefingBlock> = blocks.iter().map(|(b, _)| b.clone()).collect();
  let doc = match current {
    Some(text) => roll(text, day)?,
    None => skeleton(month, day, author, &configured),
  };
  let monday = Period::Week(day).days().0;
  let doc = fill(&doc, "today", &day_lines(events, day))?;
  let doc = fill(
    &doc,
    "tomorrow",
    &day_lines(events, day.saturating_add(1.day())),
  )?;
  let doc = fill(&doc, "week", &week_lines(events, monday))?;
  let mut doc = fill(&doc, "month", &month_lines(events, day, month))?;
  for (block, output) in blocks {
    doc = fill(&doc, &block.name, output.trim_end())?;
  }
  Ok(doc)
}

/// The window of events a briefing needs.
pub fn window(day: Date) -> (Date, Date) {
  let (first, last) = Period::Month(day).days();
  let (monday, sunday) = Period::Week(day).days();
  (
    first.min(monday).min(day.saturating_sub(1.day())),
    last.max(sunday).max(day.saturating_add(1.day())),
  )
}

/// Run a block's command; its standard output is the block.
pub fn run_block(block: &BriefingBlock) -> Result<String> {
  let (program, args) = block
    .command
    .split_first()
    .ok_or_else(|| format!("briefing block {}: empty command", block.name))?;
  let program = crate::config::expand(program)?;
  let out = std::process::Command::new(&program)
    .args(args)
    .output()
    .map_err(|e| format!("briefing block {}: {}: {e}", block.name, program.display()))?;
  if !out.status.success() {
    return Err(
      format!(
        "briefing block {}: {} failed: {}",
        block.name,
        program.display(),
        crate::clean(String::from_utf8_lossy(&out.stderr).trim())
      )
      .into(),
    );
  }
  Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;
  use jiff::tz::TimeZone;

  fn timed(title: &str, day: Date, h: i8, minutes: i64) -> crate::Result<Event> {
    let start = day.at(h, 0, 0, 0).to_zoned(TimeZone::UTC)?;
    let end = start.checked_add(minutes.minutes())?;
    Ok(event(title, When::Timed { start, end }))
  }

  fn event(title: &str, when: When) -> Event {
    Event {
      uid: title.into(),
      title: title.into(),
      when,
      calendars: vec![],
      location: None,
      link: Some(format!("busycalevent://info/{}", title.len())),
      join: None,
      recurring: false,
      response: None,
      cancelled: false,
    }
  }

  fn block() -> BriefingBlock {
    BriefingBlock {
      name: "links".into(),
      heading: "Links".into(),
      command: vec!["true".into()],
    }
  }

  #[test]
  fn day_lines_link_titles_escape_brackets_and_offer_the_call() -> crate::Result<()> {
    let day = Date::new(2026, 11, 2)?;
    let mut call = timed("[Acme] Sync", day, 9, 30)?;
    call.join = Some("https://meet.google.com/x".into());
    let away = event(
      "Away",
      When::AllDay {
        first: day,
        last: day,
      },
    );
    let lines = day_lines(&[away, call], day);
    let expected = [
      "- All day: [Away](busycalevent://info/4)".to_owned(),
      "- 09:00-09:30 [\\[Acme\\] Sync](busycalevent://info/11) · [join](https://meet.google.com/x)"
        .to_owned(),
    ]
    .join("\n");
    assert_eq!(lines, expected);
    assert_eq!(day_lines(&[], day), "- Nothing in the calendar.");
    Ok(())
  }

  #[test]
  fn a_new_day_moves_the_old_briefing_under_earlier() -> crate::Result<()> {
    let monday = Date::new(2026, 11, 2)?;
    let tuesday = monday.saturating_add(1.day());
    let blocks = vec![(block(), "- one link".to_owned())];
    let first = build(
      None,
      monday,
      "A. Writer",
      &[timed("Standup", monday, 9, 15)?],
      &blocks,
    )?;
    let written = first.replace(
      "### Yesterday (Sun 1)\n\n- \n",
      "### Yesterday (Sun 1)\n\n- hand-written\n",
    );
    let second = build(Some(&written), tuesday, "A. Writer", &[], &blocks)?;
    assert!(second.contains(&format!("## Briefing: {}", label(tuesday))));
    let earlier = second.split("## Earlier\n").nth(1).ok_or("earlier")?;
    assert!(earlier.contains(&format!("### Briefing: {}", label(monday))));
    assert!(earlier.contains("- hand-written"));
    assert!(earlier.contains("Standup"));
    assert!(!earlier.contains("<!-- gen:"));
    // The same day again rewrites only the markers.
    let again = build(Some(&second), tuesday, "A. Writer", &[], &blocks)?;
    assert_eq!(again, second);
    assert!(second.contains("<!-- gen:links -->\n- one link\n<!-- /gen:links -->"));
    Ok(())
  }

  #[test]
  fn the_month_lists_all_day_events_not_yet_over() -> crate::Result<()> {
    let today = Date::new(2026, 11, 10)?;
    let past = event(
      "Past",
      When::AllDay {
        first: Date::new(2026, 11, 2)?,
        last: Date::new(2026, 11, 3)?,
      },
    );
    let span = event(
      "Trip",
      When::AllDay {
        first: Date::new(2026, 11, 9)?,
        last: Date::new(2026, 11, 12)?,
      },
    );
    let lines = month_lines(&[past, span], today, today);
    assert_eq!(lines, "- Mon 9 to Thu 12: [Trip](busycalevent://info/4)");
    Ok(())
  }
}
