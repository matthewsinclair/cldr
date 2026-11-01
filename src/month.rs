//! `cldr month new`: start a month and archive the ones before it.
//!
//! A plan is worked out first and only then applied, so `--dry-run` shows exactly what would
//! happen. Nothing is overwritten: an existing file is left as it is, and a move onto an
//! existing file stops the plan.

use crate::Result;
use crate::config::MonthFiles;
use crate::period::Period;
use jiff::civil::Date;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
  /// A new month file from its template.
  Create { path: PathBuf, text: String },
  /// An earlier month's file into its archive folder.
  Move { from: PathBuf, to: PathBuf },
  /// A root link repointed (or a plain copy that sync made of it replaced) to `target`, a name
  /// relative to the root.
  Link { link: PathBuf, target: String },
  /// Something left alone, and why.
  Skip { path: PathBuf, why: String },
}

impl fmt::Display for Action {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let name = |p: &Path| {
      p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
    };
    match self {
      Action::Create { path, .. } => write!(f, "create  {}", name(path)),
      Action::Move { from, to } => {
        let folder = to
          .parent()
          .and_then(|p| {
            let month = p.file_name()?;
            let year = p.parent()?.file_name()?;
            Some(format!(
              "{}/{}/",
              year.to_string_lossy(),
              month.to_string_lossy()
            ))
          })
          .unwrap_or_default();
        write!(f, "move    {} -> {folder}", name(from))
      }
      Action::Link { link, target } => write!(f, "link    {} -> {target}", name(link)),
      Action::Skip { path, why } => write!(f, "leave   {} ({why})", name(path)),
    }
  }
}

use crate::names::{self, DAY_NOTES};

/// `YYYYMMDD-YYYYMMDD` for the month holding `date`.
pub fn range(date: Date) -> String {
  let (first, last) = Period::Month(date).days();
  names::span(first, last)
}

/// The month a root file belongs to: any file whose name starts with one calendar month's
/// range, whatever its kind or extension.
fn month_of(name: &str) -> Option<Date> {
  let (first, last, _) = names::parse(name)?;
  (first == first.first_of_month() && last == first.last_of_month()).then_some(first)
}

const MONTH_NAMES: [&str; 12] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/// Fill a template's `{{NAME}}` placeholders. An unknown placeholder is refused, so a template
/// written for other tooling cannot leave one behind.
pub fn interpolate(template: &str, month: Date, author: &str) -> Result<String> {
  let (first, last) = Period::Month(month).days();
  let month_name = MONTH_NAMES
    .get(usize::try_from(first.month() - 1)?)
    .copied()
    .unwrap_or_default();
  let values = [
    ("START_DATE", first.strftime("%Y%m%d").to_string()),
    ("END_DATE", last.strftime("%Y%m%d").to_string()),
    ("YEAR", first.strftime("%Y").to_string()),
    ("MONTH", first.strftime("%m").to_string()),
    ("MONTH_NAME", month_name.to_owned()),
    ("AUTHOR", author.to_owned()),
  ];
  let mut out = String::with_capacity(template.len());
  let mut rest = template;
  while let Some(open) = rest.find("{{") {
    out.push_str(&rest[..open]);
    let after = &rest[open + 2..];
    let close = after
      .find("}}")
      .ok_or_else(|| format!("unclosed placeholder: {}", crate::clean(&rest[open..])))?;
    let key = after[..close].trim();
    let value = values
      .iter()
      .find(|(k, _)| *k == key)
      .map(|(_, v)| v.as_str())
      .ok_or_else(|| format!("unknown placeholder {{{{{}}}}}", crate::clean(key)))?;
    out.push_str(value);
    rest = &after[close + 2..];
  }
  out.push_str(rest);
  Ok(out)
}

/// The note at the top of a month's Day Notes when its first week starts in the month before.
pub fn spanning_note(month: Date) -> Option<String> {
  let first = month.first_of_month();
  let (monday, sunday) = Period::Week(first).days();
  if monday == first {
    return None;
  }
  Some(format!(
    "<!-- Week {} ({} to {}) is summarised in the previous month's Day Notes -->",
    monday.iso_week_date().week(),
    monday.strftime("%-d %b"),
    sunday.strftime("%-d %b")
  ))
}

/// Put a line under the file's first `# ` heading, or at the end when it has none.
fn insert_under_title(text: &str, line: &str) -> String {
  let mut out = String::with_capacity(text.len() + line.len() + 2);
  let mut done = false;
  for l in text.split_inclusive('\n') {
    out.push_str(l);
    if !done && l.starts_with("# ") {
      if !l.ends_with('\n') {
        out.push('\n');
      }
      out.push('\n');
      out.push_str(line);
      out.push('\n');
      done = true;
    }
  }
  if !done {
    if !out.is_empty() && !out.ends_with('\n') {
      out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
  }
  out
}

pub struct Setup<'a> {
  pub root: &'a Path,
  pub templates: Option<&'a Path>,
  pub files: &'a MonthFiles,
  pub author: &'a str,
}

/// What `month new` would do to start the month holding `month`.
pub fn plan(setup: &Setup<'_>, month: Date) -> Result<Vec<Action>> {
  let month = month.first_of_month();
  let root = setup.root;
  let mut actions = Vec::new();

  // 1. The month's own files.
  let mut created = Vec::new();
  for (kind, template) in &setup.files.templates {
    let path = root.join(names::month_file(month, kind));
    if path.exists() {
      actions.push(Action::Skip {
        path,
        why: "exists".into(),
      });
      continue;
    }
    let dir = setup
      .templates
      .ok_or("journal.templates is not set, and month.templates names templates")?;
    let template_path = dir.join(template);
    let text = std::fs::read_to_string(&template_path)
      .map_err(|e| format!("{}: {e}", template_path.display()))?;
    let mut text = interpolate(&text, month, setup.author)
      .map_err(|e| format!("{}: {e}", template_path.display()))?;
    if kind == DAY_NOTES
      && let Some(note) = spanning_note(month)
    {
      text = insert_under_title(&text, &note);
    }
    created.push(path.clone());
    actions.push(Action::Create { path, text });
  }

  // 2. The links, checked before anything moves.
  for (link, kind) in &setup.files.links {
    let link_path = root.join(link);
    let target = names::month_file(month, kind);
    let target_path = root.join(&target);
    if !target_path.exists() && !created.contains(&target_path) {
      actions.push(Action::Skip {
        path: link_path,
        why: format!("no {target} yet"),
      });
      continue;
    }
    match std::fs::symlink_metadata(&link_path) {
      Ok(meta) if meta.file_type().is_symlink() => {
        if std::fs::read_link(&link_path)?.as_os_str() == target.as_str() {
          continue;
        }
      }
      Ok(meta) if meta.is_file() => {
        // Sync turned the link into a copy: it may go only if a month file holds the same text.
        let copy = std::fs::read(&link_path)?;
        let matched = month_files(root)?
          .into_iter()
          .filter(|(name, _)| names::parse_month_file(name).is_some_and(|(_, k)| k == kind))
          .any(|(name, _)| std::fs::read(root.join(name)).is_ok_and(|t| t == copy));
        if !matched {
          return Err(
            format!(
              "{link} is a plain file that matches no month's {kind}; keep what it holds, then remove it"
            )
            .into(),
          );
        }
      }
      Ok(_) => return Err(format!("{link} is neither a link nor a file").into()),
      Err(_) => {}
    }
    actions.push(Action::Link {
      link: link_path,
      target,
    });
  }

  // 3. Every earlier month's files at the root, into YYYY/YYYYMM/.
  for (name, of) in month_files(root)? {
    if of >= month {
      continue;
    }
    let to = root
      .join(of.strftime("%Y").to_string())
      .join(of.strftime("%Y%m").to_string())
      .join(&name);
    if to.exists() {
      return Err(format!("{} already exists; resolve it by hand", to.display()).into());
    }
    actions.push(Action::Move {
      from: root.join(&name),
      to,
    });
  }
  Ok(actions)
}

/// Plain files at the root named for one calendar month, with that month, in name order.
fn month_files(root: &Path) -> Result<Vec<(String, Date)>> {
  let mut out = Vec::new();
  for entry in std::fs::read_dir(root)? {
    let entry = entry?;
    if !entry.file_type()?.is_file() {
      continue;
    }
    if let Some(name) = entry.file_name().to_str()
      && let Some(of) = month_of(name)
    {
      out.push((name.to_owned(), of));
    }
  }
  out.sort();
  Ok(out)
}

fn git(root: &Path, args: &[&str]) -> Result<bool> {
  let status = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status();
  Ok(status.is_ok_and(|s| s.success()))
}

fn relative<'p>(root: &Path, path: &'p Path) -> Result<&'p str> {
  path
    .strip_prefix(root)?
    .to_str()
    .ok_or_else(|| format!("{}: not UTF-8", path.display()).into())
}

/// Carry out a plan. Moves use `git mv` for files git tracks, so history follows them.
pub fn apply(root: &Path, actions: &[Action]) -> Result<()> {
  let in_git = git(root, &["rev-parse", "--is-inside-work-tree"])?;
  for action in actions {
    match action {
      Action::Create { path, text } => {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
          .write(true)
          .create_new(true)
          .open(path)
          .map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(text.as_bytes())?;
      }
      Action::Move { from, to } => {
        if let Some(parent) = to.parent() {
          std::fs::create_dir_all(parent)?;
        }
        let (a, b) = (relative(root, from)?, relative(root, to)?);
        let tracked = in_git && git(root, &["ls-files", "--error-unmatch", "--", a])?;
        if tracked {
          if !git(root, &["mv", "--", a, b])? {
            return Err(format!("git mv {a} failed").into());
          }
        } else {
          std::fs::rename(from, to).map_err(|e| format!("{}: {e}", from.display()))?;
        }
      }
      Action::Link { link, target } => {
        if std::fs::symlink_metadata(link).is_ok() {
          std::fs::remove_file(link)?;
        }
        symlink(target, link)?;
      }
      Action::Skip { .. } => {}
    }
  }
  Ok(())
}

#[cfg(unix)]
pub fn symlink(target: &str, link: &Path) -> Result<()> {
  std::os::unix::fs::symlink(target, link).map_err(|e| format!("{}: {e}", link.display()).into())
}

#[cfg(not(unix))]
pub fn symlink(_target: &str, link: &Path) -> Result<()> {
  Err(format!("{}: links are only made on Unix", link.display()).into())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::BTreeMap;

  struct Journal {
    root: PathBuf,
    templates: PathBuf,
    files: MonthFiles,
  }

  impl Journal {
    fn new(name: &str) -> Result<Journal> {
      let base = std::env::temp_dir().join(format!("cldr-month-{name}-{}", std::process::id()));
      if base.exists() {
        std::fs::remove_dir_all(&base)?;
      }
      let root = base.join("journal");
      let templates = base.join("templates");
      std::fs::create_dir_all(&root)?;
      std::fs::create_dir_all(&templates)?;
      std::fs::write(
        templates.join("notes.md"),
        "---\nverblock: \"{{START_DATE}}:v0.1: {{AUTHOR}}\"\n---\n# {{START_DATE}}-{{END_DATE}} Day Notes\n\n##### TWIL\n",
      )?;
      std::fs::write(
        templates.join("links.md"),
        "# {{MONTH_NAME}} {{YEAR}} links\n",
      )?;
      let files = MonthFiles {
        templates: BTreeMap::from([
          (DAY_NOTES.to_owned(), "notes.md".to_owned()),
          ("link-dump".to_owned(), "links.md".to_owned()),
        ]),
        links: BTreeMap::from([
          ("day_notes.md".to_owned(), DAY_NOTES.to_owned()),
          ("briefing.md".to_owned(), "briefing".to_owned()),
        ]),
      };
      Ok(Journal {
        root,
        templates,
        files,
      })
    }

    fn setup(&self) -> Setup<'_> {
      Setup {
        root: &self.root,
        templates: Some(&self.templates),
        files: &self.files,
        author: "A. Writer",
      }
    }

    fn write(&self, name: &str, text: &str) -> Result<()> {
      Ok(std::fs::write(self.root.join(name), text)?)
    }

    fn done(self) -> Result<()> {
      if let Some(base) = self.root.parent() {
        std::fs::remove_dir_all(base)?;
      }
      Ok(())
    }
  }

  #[test]
  fn placeholders_are_filled_and_unknown_ones_refused() -> Result<()> {
    let month = Date::new(2026, 2, 10)?;
    let text = interpolate(
      "{{START_DATE}}-{{END_DATE}} {{MONTH_NAME}} {{AUTHOR}}",
      month,
      "Me",
    )?;
    assert_eq!(text, format!("{} February Me", range(month)));
    assert!(interpolate("{{DOW_ROW}}", month, "Me").is_err());
    assert!(interpolate("{{START_DATE", month, "Me").is_err());
    Ok(())
  }

  #[test]
  fn only_a_month_that_starts_mid_week_gets_the_spanning_note() -> Result<()> {
    let starts_on_monday = Date::new(2026, 6, 1)?;
    assert_eq!(starts_on_monday.weekday(), jiff::civil::Weekday::Monday);
    assert!(spanning_note(starts_on_monday).is_none());
    let mid_week = Date::new(2026, 11, 1)?;
    let note = spanning_note(mid_week).ok_or("note")?;
    let week = Period::Week(mid_week).days().0.iso_week_date().week();
    assert!(note.contains(&format!("Week {week} ")));
    Ok(())
  }

  #[test]
  fn a_new_month_creates_archives_and_relinks() -> Result<()> {
    let j = Journal::new("turn")?;
    let november = Date::new(2026, 11, 1)?;
    let october = Date::new(2026, 10, 1)?;
    let september = Date::new(2026, 9, 1)?;
    let old: Vec<String> = [
      names::month_file(october, DAY_NOTES),
      names::month_file(october, "briefing"),
      names::month_file(september, "briefing"),
    ]
    .into();
    for name in &old {
      j.write(name, name)?;
    }
    j.write(&names::month_file(november, "briefing"), "already started")?;
    // Sync made a copy of the link; it matches October's briefing, so it may go.
    j.write("briefing.md", &names::month_file(october, "briefing"))?;
    j.write("unrelated.md", "mine")?;

    let actions = plan(&j.setup(), november)?;
    apply(&j.root, &actions)?;

    let notes = std::fs::read_to_string(j.root.join(names::month_file(november, DAY_NOTES)))?;
    assert!(notes.contains(&format!("# {} Day Notes\n\n<!-- Week", range(november))));
    assert!(notes.contains("A. Writer"));
    assert!(
      j.root
        .join(names::month_file(november, "link-dump"))
        .exists()
    );
    assert_eq!(
      std::fs::read_to_string(j.root.join(names::month_file(november, "briefing")))?,
      "already started"
    );
    for name in &old {
      let of = month_of(name).ok_or("month")?;
      let archived = j
        .root
        .join(of.strftime("%Y").to_string())
        .join(of.strftime("%Y%m").to_string())
        .join(name);
      assert_eq!(std::fs::read_to_string(archived)?, *name);
      assert!(!j.root.join(name).exists());
    }
    for (link, kind) in &j.files.links {
      assert_eq!(
        std::fs::read_link(j.root.join(link))?,
        PathBuf::from(names::month_file(november, kind))
      );
    }
    assert!(j.root.join("unrelated.md").exists());

    // A second run finds nothing to do.
    let again = plan(&j.setup(), november)?;
    assert!(
      again.iter().all(|a| matches!(a, Action::Skip { .. })),
      "{again:?}"
    );
    j.done()
  }

  #[test]
  fn a_plain_copy_of_a_link_that_matches_nothing_stops_the_plan() -> Result<()> {
    let j = Journal::new("copy")?;
    let november = Date::new(2026, 11, 1)?;
    j.write(&names::month_file(november, "briefing"), "new")?;
    j.write("briefing.md", "written into the copy, nowhere else")?;
    assert!(plan(&j.setup(), november).is_err());
    j.done()
  }

  #[test]
  fn a_move_onto_an_archived_file_stops_the_plan() -> Result<()> {
    let j = Journal::new("clash")?;
    let october = Date::new(2026, 10, 1)?;
    let name = names::month_file(october, DAY_NOTES);
    j.write(&name, "root copy")?;
    let folder = j.root.join("2026").join("202610");
    std::fs::create_dir_all(&folder)?;
    std::fs::write(folder.join(&name), "archived copy")?;
    assert!(plan(&j.setup(), Date::new(2026, 11, 1)?).is_err());
    j.done()
  }

  #[test]
  fn names_that_do_not_span_one_month_are_not_month_files() -> Result<()> {
    assert!(month_of("20261001-20261031-day-notes.md").is_some());
    assert!(month_of("20261001-20261031-activity-log.md").is_some());
    assert!(month_of("20261026-20261101-w44-calendar.csv").is_none());
    assert!(month_of("2026 Hours (YTD).md").is_none());
    assert!(month_of("20261001-20261031 Day Notes.md").is_none());
    Ok(())
  }
}
