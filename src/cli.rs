use crate::Result;
use crate::calendar::busycal::BusyCal;
use crate::calendar::{CalendarSource, Filter};
use crate::config::{self, Config};
use crate::doctor;
use crate::export::{self, Period};
use jiff::Zoned;
use jiff::civil::Date;
use std::path::PathBuf;

const HELP: &str = "Cldr: a re-runnable renderer for a plain-text time journal

Usage: cldr [--config PATH] COMMAND

  export week|month [DATE] [--stdout] [--force] [--dropped]
                         The calendar as CSV for the week or month holding DATE
                         (default today), written to the journal's YYYY/YYYYMM/
                         folder (a week files under its Sunday's month)
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

fn export_command(explicit: Option<PathBuf>, mut args: Vec<String>) -> Result<()> {
  let to_stdout = take_flag(&mut args, "--stdout");
  let force = take_flag(&mut args, "--force");
  let dropped = take_flag(&mut args, "--dropped");
  let loaded = Config::load(explicit.as_deref())?;
  let config = loaded.config;
  let zone = config.zone()?;
  let kind = if args.is_empty() {
    String::new()
  } else {
    args.remove(0)
  };
  let date = match args.first() {
    Some(text) => text
      .parse::<Date>()
      .map_err(|e| format!("{text}: expected YYYY-MM-DD ({e})"))?,
    None => Zoned::now().with_time_zone(zone.clone()).date(),
  };
  if args.len() > 1 {
    return Err(format!("unexpected arguments: {}", args[1..].join(" ")).into());
  }
  let period = match kind.as_str() {
    "week" => Period::Week(date),
    "month" => Period::Month(date),
    _ => return Err("cldr export week|month [DATE]".into()),
  };
  if config.calendar.source != "busycal" {
    return Err(
      format!(
        "calendar.source {}: only busycal is supported",
        config.calendar.source
      )
      .into(),
    );
  }
  let source = BusyCal::open(&config.store_path()?, zone, &config.calendar.me)?;
  let filter = Filter {
    include: config.calendar.include.clone(),
    exclude: config.calendar.exclude.clone(),
    dropped,
  };
  let (first, last) = period.days();
  let events = source.events(first, last, &filter)?;
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
