//! The cldr binary run over a generated journal, end to end, beside the unit tests.
//!
//! Each test builds its own sandbox in the temporary directory: a journal, the config cldr finds
//! through XDG_CONFIG_HOME, and a synthetic BusyCal store saved from the unit tests' one
//! builder. HOME and the XDG variables point inside the sandbox and CLDR_CONFIG is removed, so
//! nothing of a developer's own setup can reach a test. Every date is in a past month, or named,
//! so today's date cannot change what a command writes. All data is synthetic.

use cldr::Result;
use cldr::calendar::busycal::fixture::{Spec, Store};
use cldr::period::Period;
use cldr::{briefing, names};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ZONE: &str = "Europe/London";
const AUTHOR: &str = "A. Writer";
const ME: &str = "me@example.com";
/// A tag as the Day Notes write it, and as the config spells it.
const TAG: &str = "@acme";
const SPELT: &str = "@Acme";
/// What the briefing's one block prints.
const PROJECT: &str = "- @acme: the data migration";
/// The Time Log rows of the month the tests read: day, time, hours, what.
const ROWS: [(i8, &str, f64, &str); 3] = [
  (20, "09:00-17:00", 8.0, "the migration"),
  (13, "09:00-12:00", 3.0, "planning"),
  (6, "09:00-10:00", 1.0, "a call"),
];

/// A past month, so today's date cannot change what render, check and data make of it.
fn month() -> Result<Date> {
  Ok(Date::new(2026, 10, 1)?)
}

/// The month as the command line names it.
fn month_arg(month: Date) -> String {
  month.strftime("%Y-%m").to_string()
}

struct Sandbox {
  root: PathBuf,
}

impl Sandbox {
  fn new(name: &str) -> Result<Sandbox> {
    let root = std::env::temp_dir().join(format!("cldr-cli-test-{name}-{}", std::process::id()));
    if root.exists() {
      std::fs::remove_dir_all(&root)?;
    }
    let sandbox = Sandbox { root };
    for dir in [
      sandbox.home(),
      sandbox.config_home().join("cldr"),
      sandbox.state_home(),
      sandbox.cache_home(),
      sandbox.journal(),
      sandbox.templates(),
    ] {
      std::fs::create_dir_all(dir)?;
    }
    sandbox.write_config()?;
    Ok(sandbox)
  }

  fn home(&self) -> PathBuf {
    self.root.join("home")
  }

  fn config_home(&self) -> PathBuf {
    self.root.join("config")
  }

  fn state_home(&self) -> PathBuf {
    self.root.join("state")
  }

  fn cache_home(&self) -> PathBuf {
    self.root.join("cache")
  }

  fn journal(&self) -> PathBuf {
    self.root.join("journal")
  }

  fn templates(&self) -> PathBuf {
    self.root.join("templates")
  }

  fn store(&self) -> PathBuf {
    self.root.join("store.bsql")
  }

  /// The config, where cldr looks for it through XDG_CONFIG_HOME.
  fn write_config(&self) -> Result<()> {
    let text = format!(
      r#"[journal]
root = "{journal}"
author = "{AUTHOR}"
timezone = "{ZONE}"
templates = "{templates}"

[calendar]
source = "busycal"
store = "{store}"
me = ["{ME}"]

[tags]
"{TAG}" = "{SPELT}"

[month.templates]
"{day_notes}" = "{day_notes}.md"

[month.links]
"day_notes.md" = "{day_notes}"
"daily_briefing.md" = "{daily_briefing}"

[[briefing.blocks]]
name = "projects"
heading = "Projects"
command = ["/bin/echo", "{PROJECT}"]
"#,
      journal = self.journal().display(),
      templates = self.templates().display(),
      store = self.store().display(),
      day_notes = names::DAY_NOTES,
      daily_briefing = briefing::DAILY_BRIEFING,
    );
    std::fs::write(self.config_home().join("cldr").join("config.toml"), text)?;
    Ok(())
  }

  /// cldr, run with the sandbox's environment.
  fn cldr(&self, args: &[&str]) -> Result<Output> {
    Ok(
      Command::new(env!("CARGO_BIN_EXE_cldr"))
        .args(args)
        .env_remove("CLDR_CONFIG")
        .env("HOME", self.home())
        .env("XDG_CONFIG_HOME", self.config_home())
        .env("XDG_STATE_HOME", self.state_home())
        .env("XDG_CACHE_HOME", self.cache_home())
        .output()?,
    )
  }

  /// cldr, which must succeed; its standard output.
  fn ok(&self, args: &[&str]) -> Result<String> {
    let out = self.cldr(args)?;
    if !out.status.success() {
      return Err(
        format!(
          "cldr {} failed: {}",
          args.join(" "),
          String::from_utf8_lossy(&out.stderr)
        )
        .into(),
      );
    }
    Ok(String::from_utf8(out.stdout)?)
  }

  fn remove(self) -> Result<()> {
    std::fs::remove_dir_all(&self.root)?;
    Ok(())
  }
}

/// A month's Day Notes as the user writes them: the title, then each day of `rows`, newest first.
fn day_notes(month: Date, rows: &[(i8, &str, f64, &str)]) -> Result<String> {
  let (first, last) = Period::Month(month).days();
  let mut text = format!("# {} Day Notes\n\n", names::span(first, last));
  let mut days = rows.to_vec();
  days.sort_by_key(|r| std::cmp::Reverse(r.0));
  for (day, time, hours, what) in days {
    let date = Date::new(month.year(), month.month(), day)?;
    text.push_str(&format!(
      "#### ---------------\n#### {} {}\n\n#### Time Log\n\n\
       | Project | Time | Hours | What |\n|---|:---:|---:|---|\n\
       | {TAG} | {time} | {hours:.1} | {what} |\n\nNotes for the day.\n\n",
      date.strftime("%Y%m%d"),
      date.strftime("%A"),
    ));
  }
  Ok(text)
}

/// Write the month's Day Notes at the journal's root; their path and text.
fn write_day_notes(sandbox: &Sandbox, month: Date) -> Result<(PathBuf, String)> {
  let path = sandbox
    .journal()
    .join(names::month_file(month, names::DAY_NOTES));
  let text = day_notes(month, &ROWS)?;
  std::fs::write(&path, &text)?;
  Ok((path, text))
}

/// A store holding one calendar and an hour-long event for each title, at its day and hour.
fn store(events: &[(&str, Date, i8)]) -> Result<Store> {
  let mut store = Store::new()?;
  let calendar = store.calendar("Work", ME, false)?;
  let zone = TimeZone::get(ZONE)?;
  for (title, day, hour) in events {
    store.event(Spec {
      calendar,
      title,
      start: day.at(*hour, 0, 0, 0).to_zoned(zone.clone())?,
      end: day.at(*hour + 1, 0, 0, 0).to_zoned(zone.clone())?,
      all_day: false,
      uid: title,
      status: None,
      rule: None,
      original: None,
    })?;
  }
  Ok(store)
}

/// Every file under a folder, with its bytes, in path order.
fn tree(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
  let mut found = Vec::new();
  let mut folders = vec![root.to_path_buf()];
  while let Some(folder) = folders.pop() {
    for entry in std::fs::read_dir(&folder)? {
      let path = entry?.path();
      if path.is_dir() {
        folders.push(path);
      } else {
        let bytes = std::fs::read(&path)?;
        found.push((path, bytes));
      }
    }
  }
  found.sort();
  Ok(found)
}

// ST0002 AT-06
#[test]
fn paths_resolve_inside_the_sandbox() -> Result<()> {
  let sandbox = Sandbox::new("paths")?;
  let out = sandbox.ok(&["paths"])?;
  let root = sandbox.root.display().to_string();
  assert!(!out.is_empty());
  for line in out.lines() {
    assert!(line.contains(&root), "outside the sandbox: {line}");
  }
  sandbox.remove()
}

// ST0002 AT-01
#[test]
fn render_twice_changes_nothing_and_keeps_the_hand_written_text() -> Result<()> {
  let sandbox = Sandbox::new("render")?;
  let month = month()?;
  let (path, written) = write_day_notes(&sandbox, month)?;
  let arg = month_arg(month);
  let args = ["render", "month", arg.as_str(), "--no-calendar"];
  sandbox.ok(&args)?;
  let once = std::fs::read_to_string(&path)?;
  assert_ne!(once, written, "render wrote no sections");
  sandbox.ok(&args)?;
  assert_eq!(std::fs::read_to_string(&path)?, once);
  // Every line the user wrote is still there, in its order.
  let mut rendered = once.lines();
  for line in written.lines() {
    assert!(rendered.any(|l| l == line), "lost or moved: {line:?}");
  }
  let json = path.with_file_name(names::month_file_with(month, names::DATA, "json"));
  assert!(json.is_file(), "no {}", json.display());
  sandbox.remove()
}

// ST0002 AT-02
#[test]
fn data_reads_the_month_and_check_writes_nothing() -> Result<()> {
  let sandbox = Sandbox::new("data")?;
  let month = month()?;
  write_day_notes(&sandbox, month)?;
  let before = tree(&sandbox.journal())?;
  let arg = month_arg(month);
  let out = sandbox.ok(&["data", "month", arg.as_str(), "--no-calendar"])?;
  let data: serde_json::Value = serde_json::from_str(&out)?;
  let hours: f64 = ROWS.iter().map(|r| r.2).sum();
  assert_eq!(data["hours"].as_f64(), Some(hours));
  assert_eq!(data["by_tag"][SPELT].as_f64(), Some(hours));
  sandbox.ok(&["check", "month", arg.as_str(), "--no-calendar"])?;
  assert_eq!(tree(&sandbox.journal())?, before);
  sandbox.remove()
}

// ST0002 AT-05
#[test]
fn export_writes_the_months_csv_and_replaces_it_only_with_force() -> Result<()> {
  let sandbox = Sandbox::new("export")?;
  let month = month()?;
  let day = Date::new(month.year(), month.month(), ROWS[0].0)?;
  let (first, second) = ("Planning", "Review");
  store(&[(first, day, 10)])?.save(&sandbox.store())?;
  let arg = month_arg(month);
  let args = ["export", "month", arg.as_str()];
  sandbox.ok(&args)?;
  let csv = Period::Month(month).file(&sandbox.journal());
  let exported = std::fs::read_to_string(&csv)?;
  assert!(exported.contains(first), "{exported}");
  // The store changes; without --force the CSV already there is refused and kept.
  store(&[(first, day, 10), (second, day, 14)])?.save(&sandbox.store())?;
  let refused = sandbox.cldr(&args)?;
  let error = String::from_utf8_lossy(&refused.stderr);
  assert!(!refused.status.success());
  assert!(
    error.contains(&csv.display().to_string()) && error.contains("--force"),
    "{error}"
  );
  assert_eq!(std::fs::read_to_string(&csv)?, exported);
  sandbox.ok(&["export", "month", arg.as_str(), "--force"])?;
  let replaced = std::fs::read_to_string(&csv)?;
  assert!(
    replaced.contains(first) && replaced.contains(second),
    "{replaced}"
  );
  sandbox.remove()
}

// ST0002 AT-03
#[test]
fn briefing_fills_the_day_and_its_block_and_keeps_the_users_text() -> Result<()> {
  let sandbox = Sandbox::new("briefing")?;
  let day = Date::new(month()?.year(), month()?.month(), ROWS[0].0)?;
  let title = "Planning";
  store(&[(title, day, 10)])?.save(&sandbox.store())?;
  let arg = day.to_string();
  sandbox.ok(&["briefing", arg.as_str()])?;
  let name = names::month_file(day, briefing::DAILY_BRIEFING);
  let path = sandbox.journal().join(&name);
  let text = std::fs::read_to_string(&path)?;
  assert!(text.contains(title), "{text}");
  assert!(text.contains(PROJECT), "{text}");
  // The root's link follows the month's briefing.
  let link = std::fs::read_link(sandbox.journal().join("daily_briefing.md"))?;
  assert_eq!(link, PathBuf::from(&name));
  // The same day again rewrites only between the markers: a line of the user's survives.
  let mine = format!("{text}\nA note of my own.\n");
  std::fs::write(&path, &mine)?;
  sandbox.ok(&["briefing", arg.as_str()])?;
  assert_eq!(std::fs::read_to_string(&path)?, mine);
  sandbox.remove()
}

// ST0002 AT-04
#[test]
fn month_new_starts_the_month_archives_the_last_and_repoints_the_link() -> Result<()> {
  let sandbox = Sandbox::new("month")?;
  let last = month()?;
  let next = last.last_of_month().tomorrow()?;
  std::fs::write(
    sandbox.templates().join(format!("{}.md", names::DAY_NOTES)),
    "# {{START_DATE}}-{{END_DATE}} Day Notes\n\nBy {{AUTHOR}}.\n",
  )?;
  let (old, _) = write_day_notes(&sandbox, last)?;
  let link = sandbox.journal().join("day_notes.md");
  std::os::unix::fs::symlink(names::month_file(last, names::DAY_NOTES), &link)?;
  sandbox.ok(&["month", "new", month_arg(next).as_str()])?;
  let name = names::month_file(next, names::DAY_NOTES);
  let started = std::fs::read_to_string(sandbox.journal().join(&name))?;
  let (first, final_day) = Period::Month(next).days();
  assert!(
    started.starts_with(&format!("# {} Day Notes\n", names::span(first, final_day))),
    "{started}"
  );
  assert!(started.contains(&format!("By {AUTHOR}.")), "{started}");
  let archived = sandbox
    .journal()
    .join(last.strftime("%Y").to_string())
    .join(last.strftime("%Y%m").to_string())
    .join(old.file_name().unwrap_or_default());
  assert!(archived.is_file(), "not archived: {}", archived.display());
  assert!(!old.exists(), "still at the root: {}", old.display());
  assert_eq!(std::fs::read_link(&link)?, PathBuf::from(&name));
  sandbox.remove()
}
