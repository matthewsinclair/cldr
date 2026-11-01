use crate::Result;
use crate::calendar::busycal::BusyCal;
use crate::calendar::{CalendarSource, Filter};
use crate::config::{self, Config};
use crate::doctor;
use crate::export::{self, Period};
use crate::zones::Zones;
use crate::{check, journal, legacy, model, month, names, render};
use jiff::ToSpan;
use jiff::civil::Date;
use std::path::PathBuf;

const HELP: &str = "Cldr: a re-runnable renderer for a plain-text time journal

Usage: cldr [--config PATH] COMMAND

  export week|month [DATE] [--stdout] [--force] [--dropped]
                         The calendar as CSV for the week or month holding DATE
                         (default today), written to the journal's YYYY/YYYYMM/
                         folder (a week files under its Sunday's month)
  data week|month|year [DATE] [--no-calendar]
                         Hours, entries, weeks, months and flags as JSON, from
                         the Day Notes, with the calendar for days without them
  check week|month|year [DATE] [--no-calendar] [--activity-logs] [--json]
                         The totals and every flag, for reading; with
                         --activity-logs, each day and tag that differs from
                         the old Activity Logs. Writes nothing
  render week|month [DATE] [--dry-run] [--no-calendar]
                         Write the generated sections into the month's Day
                         Notes (only between cldr's markers) and the month's
                         JSON beside it; a week renders into the file of the
                         month it starts in (default: last week, this month)
  month new [YYYY-MM] [--dry-run]
                         Start the month (default this month) from the
                         templates, move earlier months' files into
                         YYYY/YYYYMM/ (git mv in a work tree), repoint the
                         root links
  doctor                 Check the config, the journal and calendar access
  paths                  Show resolved paths
  help                   Show this help
  --version              Show the version

Config: $XDG_CONFIG_HOME/cldr/config.toml (default ~/.config/cldr/config.toml)
Override: CLDR_CONFIG or --config PATH. The calendar store is only ever read.";

fn take_value(args: &mut Vec<String>, flag: &str) -> Result<Option<String>> {
  match args.iter().position(|a| a == flag) {
    Some(index) => {
      args.remove(index);
      if index >= args.len() || args[index].starts_with("--") {
        return Err(format!("{flag} needs a value").into());
      }
      Ok(Some(args.remove(index)))
    }
    None => Ok(None),
  }
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
  match args.iter().position(|a| a == flag) {
    Some(index) => {
      args.remove(index);
      true
    }
    None => false,
  }
}

pub fn run(mut args: Vec<String>) -> Result<()> {
  let explicit = take_value(&mut args, "--config")?
    .map(|p| config::expand(&p))
    .transpose()?;
  let command = if args.is_empty() {
    "help".to_owned()
  } else {
    args.remove(0)
  };
  match command.as_str() {
    "help" | "--help" | "-h" => {
      println!("{HELP}");
      Ok(())
    }
    "--version" | "-V" => {
      println!("cldr {}", env!("CARGO_PKG_VERSION"));
      Ok(())
    }
    "paths" => paths(explicit),
    "doctor" => doctor_command(explicit),
    "export" => export_command(explicit, args),
    "data" => data_command(explicit, args),
    "check" => check_command(explicit, args),
    "render" => render_command(explicit, args),
    "month" => month_command(explicit, args),
    other => Err(format!("unknown command {other}; see cldr help").into()),
  }
}

fn paths(explicit: Option<PathBuf>) -> Result<()> {
  let loaded = Config::load(explicit.as_deref())?;
  println!("config   {}", loaded.path.display());
  println!(
    "state    {}",
    config::xdg_path("XDG_STATE_HOME", ".local/state")?.display()
  );
  println!(
    "cache    {}",
    config::xdg_path("XDG_CACHE_HOME", ".cache")?.display()
  );
  match loaded.config.journal_root()? {
    Some(root) => println!("journal  {}", root.display()),
    None => println!("journal  (journal.root not set)"),
  }
  println!("calendar {}", loaded.config.store_path()?.display());
  Ok(())
}

fn doctor_command(explicit: Option<PathBuf>) -> Result<()> {
  let loaded = Config::load(explicit.as_deref())?;
  let checks = doctor::run(&loaded);
  for check in &checks {
    println!(
      "{} {:9} {}",
      if check.ok { "ok" } else { "!!" },
      check.what,
      crate::clean(&check.detail)
    );
  }
  if checks.iter().all(|c| c.ok) {
    Ok(())
  } else {
    Err("some checks failed".into())
  }
}

/// The period an argument list names: `week|month|year [DATE]`, the date defaulting to today.
fn period_args(args: &mut Vec<String>, zones: &Zones, usage: &str) -> Result<Period> {
  let kind = if args.is_empty() {
    String::new()
  } else {
    args.remove(0)
  };
  let date = match args.first() {
    // A month may be named by its YYYY-MM.
    Some(text) if text.len() == "YYYY-MM".len() => format!("{text}-01")
      .parse::<Date>()
      .map_err(|e| format!("{text}: expected YYYY-MM or YYYY-MM-DD ({e})"))?,
    Some(text) => text
      .parse::<Date>()
      .map_err(|e| format!("{text}: expected YYYY-MM-DD ({e})"))?,
    None => today(zones),
  };
  if args.len() > 1 {
    return Err(format!("unexpected arguments: {}", args[1..].join(" ")).into());
  }
  match kind.as_str() {
    "week" => Ok(Period::Week(date)),
    "month" => Ok(Period::Month(date)),
    "year" if usage.contains("year") => Ok(Period::Year(date)),
    _ => Err(usage.to_owned().into()),
  }
}

fn today(zones: &Zones) -> Date {
  zones.date_of(jiff::Timestamp::now())
}

fn open_calendar(config: &Config, zones: Zones) -> Result<BusyCal> {
  if config.calendar.source != "busycal" {
    return Err(
      format!(
        "calendar.source {}: only busycal is supported",
        config.calendar.source
      )
      .into(),
    );
  }
  BusyCal::open(&config.store_path()?, zones, &config.calendar.me)
}

fn filter(config: &Config, dropped: bool) -> Filter {
  Filter {
    include: config.calendar.include.clone(),
    exclude: config.calendar.exclude.clone(),
    dropped,
  }
}

fn export_command(explicit: Option<PathBuf>, mut args: Vec<String>) -> Result<()> {
  let to_stdout = take_flag(&mut args, "--stdout");
  let force = take_flag(&mut args, "--force");
  let dropped = take_flag(&mut args, "--dropped");
  let config = Config::load(explicit.as_deref())?.config;
  let zones = config.zones()?;
  let period = period_args(&mut args, &zones, "cldr export week|month [DATE]")?;
  let source = open_calendar(&config, zones)?;
  let (first, last) = period.days();
  let events = source.events(first, last, &filter(&config, dropped))?;
  let text = export::csv(&events);
  if to_stdout {
    print!("{text}");
    return Ok(());
  }
  let root = config
    .journal_root()?
    .ok_or("journal.root is not set; use --stdout or set it in the config")?;
  let path = period.file(&root);
  if path.exists() && !force {
    return Err(format!("{} exists; --force to replace it", path.display()).into());
  }
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(&path, text)?;
  println!("{} ({} events)", path.display(), events.len());
  Ok(())
}

/// The model for a period, as `data` and `check` both build it.
fn build_data(
  config: &Config,
  period: Period,
  no_calendar: bool,
) -> Result<(model::Data, PathBuf)> {
  let zones = config.zones()?;
  let root = config
    .journal_root()?
    .ok_or("journal.root is not set in the config")?;
  let (first, last) = period.days();
  let notes = journal::load(&root, first, last)?;
  let today = today(&zones);
  let mut unread = None;
  let events = if no_calendar {
    None
  } else {
    // Whole weeks and a day either side, as the model reads them.
    let from = Period::Week(first).days().0.saturating_sub(1.day());
    let to = Period::Week(last).days().1.saturating_add(1.day());
    match open_calendar(config, zones).and_then(|c| c.events(from, to, &filter(config, false))) {
      Ok(events) => Some(events),
      Err(e) => {
        unread = Some(e.to_string());
        None
      }
    }
  };
  let mut data = model::build(&model::Inputs {
    period,
    notes: &notes,
    calendar: events.as_deref(),
    tags: &config.tags,
    bill: &config.bill,
    rules: &config.rules,
    today,
  });
  if let Some(reason) = unread {
    data.flags.insert(
      0,
      model::Flag {
        kind: "calendar_unread",
        date: None,
        message: crate::clean(&reason),
        file: None,
        line: None,
      },
    );
  }
  Ok((data, root))
}

fn data_command(explicit: Option<PathBuf>, mut args: Vec<String>) -> Result<()> {
  let no_calendar = take_flag(&mut args, "--no-calendar");
  let config = Config::load(explicit.as_deref())?.config;
  let zones = config.zones()?;
  let period = period_args(
    &mut args,
    &zones,
    "cldr data week|month|year [DATE] [--no-calendar]",
  )?;
  let (data, _) = build_data(&config, period, no_calendar)?;
  println!("{}", serde_json::to_string_pretty(&data)?);
  Ok(())
}

fn check_command(explicit: Option<PathBuf>, mut args: Vec<String>) -> Result<()> {
  let no_calendar = take_flag(&mut args, "--no-calendar");
  let against_logs = take_flag(&mut args, "--activity-logs");
  let as_json = take_flag(&mut args, "--json");
  let config = Config::load(explicit.as_deref())?.config;
  let zones = config.zones()?;
  let period = period_args(
    &mut args,
    &zones,
    "cldr check week|month|year [DATE] [--no-calendar] [--activity-logs] [--json]",
  )?;
  let (data, root) = build_data(&config, period, no_calendar)?;
  let comparison = if against_logs {
    let logged = legacy::load(&root, data.first, data.last)?;
    let book = model::Rulebook::new(&config.tags, &config.bill, &config.rules);
    Some(check::compare(&data, &logged, &book))
  } else {
    None
  };
  if as_json {
    let out = serde_json::json!({ "flags": data.flags, "activity_logs": comparison });
    println!("{}", serde_json::to_string_pretty(&out)?);
  } else {
    print!("{}", check::report(&data, comparison.as_ref()));
  }
  Ok(())
}

fn month_command(explicit: Option<PathBuf>, mut args: Vec<String>) -> Result<()> {
  let dry_run = take_flag(&mut args, "--dry-run");
  let usage = "cldr month new [YYYY-MM] [--dry-run]";
  if args.first().map(String::as_str) != Some("new") || args.len() > 2 {
    return Err(usage.into());
  }
  let config = Config::load(explicit.as_deref())?.config;
  let zones = config.zones()?;
  let month = match args.get(1) {
    Some(text) => format!("{text}-01")
      .parse::<Date>()
      .map_err(|e| format!("{text}: expected YYYY-MM ({e})"))?,
    None => today(&zones),
  };
  let root = config
    .journal_root()?
    .ok_or("journal.root is not set in the config")?;
  let templates = config.templates_dir()?;
  let setup = month::Setup {
    root: &root,
    templates: templates.as_deref(),
    files: &config.month,
    author: config.journal.author.as_deref().unwrap_or_default(),
  };
  let actions = month::plan(&setup, month)?;
  for action in &actions {
    println!("{}", crate::clean(&action.to_string()));
  }
  if dry_run {
    println!("(dry run: nothing changed)");
    return Ok(());
  }
  month::apply(&root, &actions)
}

fn render_command(explicit: Option<PathBuf>, mut args: Vec<String>) -> Result<()> {
  let dry_run = take_flag(&mut args, "--dry-run");
  let no_calendar = take_flag(&mut args, "--no-calendar");
  let config = Config::load(explicit.as_deref())?.config;
  let zones = config.zones()?;
  let usage = "cldr render week|month [DATE] [--dry-run] [--no-calendar]";
  // A week with no date is last week.
  if args.len() == 1 && args[0] == "week" {
    args.push(today(&zones).saturating_sub(7.days()).to_string());
  }
  let period = period_args(&mut args, &zones, usage)?;
  let (month, week) = match period {
    Period::Week(_) => {
      let monday = period.days().0;
      (monday.first_of_month(), Some(model::iso_week(monday)))
    }
    Period::Month(d) => (d.first_of_month(), None),
    Period::Year(_) => return Err(usage.into()),
  };
  let (mut data, root) = build_data(&config, Period::Month(month), no_calendar)?;
  let (first, last) = Period::Month(month).days();
  let path = journal::day_notes_files(&root)?
    .into_iter()
    .find(|(_, a, b)| *a == first && *b == last)
    .map(|(p, _, _)| p)
    .ok_or_else(|| {
      format!(
        "no Day Notes for {} under {}",
        month.strftime("%Y-%m"),
        root.display()
      )
    })?;
  let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
  let scope = match &week {
    Some(id) => render::Scope::Week(id),
    None => render::Scope::Month,
  };
  let rendered = render::render(&text, month, &data, today(&zones), scope);
  model::relative_paths(&mut data, &root);
  let json = serde_json::to_string_pretty(&data)? + "\n";
  let json_path = path.with_file_name(names::month_file_with(month, names::DATA, "json"));
  let shown = |p: &std::path::Path| p.strip_prefix(&root).unwrap_or(p).display().to_string();
  if dry_run {
    let state = if rendered == text {
      "unchanged"
    } else {
      "would change"
    };
    println!("{} {state}", shown(&path));
    println!("{} would be written", shown(&json_path));
    return Ok(());
  }
  if rendered != text {
    write_atomically(&path, &rendered)?;
    println!("{} rendered", shown(&path));
  } else {
    println!("{} unchanged", shown(&path));
  }
  write_atomically(&json_path, &json)?;
  println!("{} written", shown(&json_path));
  Ok(())
}

/// Write through a temporary file beside the target, so a reader never sees half a file.
fn write_atomically(path: &std::path::Path, text: &str) -> Result<()> {
  let mut tmp = path.as_os_str().to_owned();
  tmp.push(".cldr-tmp");
  let tmp = PathBuf::from(tmp);
  std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
  std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
  Ok(())
}
