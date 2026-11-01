use crate::Result;
use crate::zones::Zones;
use jiff::tz::TimeZone;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

const APP: &str = "cldr";

/// The user's configuration. Everything personal lives here, never in the repository.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
  #[serde(default)]
  pub journal: Journal,
  #[serde(default)]
  pub calendar: Calendar,
  #[serde(default)]
  pub tags: BTreeMap<String, String>,
  #[serde(default)]
  pub rules: Rules,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
  pub root: Option<String>,
  pub author: Option<String>,
  /// An IANA zone name; the machine's zone when absent.
  pub timezone: Option<String>,
  pub templates: Option<String>,
  /// Where the user was: from each date on, the wall clock is in this zone.
  #[serde(default)]
  pub zones: Vec<ZoneFrom>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneFrom {
  pub from: jiff::civil::Date,
  pub zone: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calendar {
  #[serde(default = "default_source")]
  pub source: String,
  /// Overrides the store's standard location.
  pub store: Option<String>,
  /// Substrings of "calendar (account)"; empty means every calendar.
  #[serde(default)]
  pub include: Vec<String>,
  #[serde(default)]
  pub exclude: Vec<String>,
  /// The user's own addresses, to read their reply to an invitation.
  #[serde(default)]
  pub me: Vec<String>,
}

impl Default for Calendar {
  fn default() -> Self {
    Calendar {
      source: default_source(),
      store: None,
      include: Vec::new(),
      exclude: Vec::new(),
      me: Vec::new(),
    }
  }
}

fn default_source() -> String {
  "busycal".to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
  #[serde(default)]
  pub exclude: Vec<String>,
  #[serde(default)]
  pub zero_hours: Vec<String>,
  #[serde(default)]
  pub weekday_only: Vec<String>,
  #[serde(default = "default_min_weekday_hours")]
  pub min_weekday_hours: f64,
  #[serde(default = "default_max_gap_hours")]
  pub max_gap_hours: f64,
}

impl Default for Rules {
  fn default() -> Self {
    Rules {
      exclude: Vec::new(),
      zero_hours: Vec::new(),
      weekday_only: Vec::new(),
      min_weekday_hours: default_min_weekday_hours(),
      max_gap_hours: default_max_gap_hours(),
    }
  }
}

fn default_min_weekday_hours() -> f64 {
  8.0
}

fn default_max_gap_hours() -> f64 {
  1.0
}

/// Where the configuration file is, and whether it exists.
#[derive(Debug)]
pub struct Loaded {
  pub path: PathBuf,
  pub exists: bool,
  pub config: Config,
}

impl Config {
  pub fn parse(text: &str) -> Result<Config> {
    Ok(toml::from_str(text)?)
  }

  /// Load from an explicit path, `CLDR_CONFIG`, or the XDG default. A missing file is an empty config.
  pub fn load(explicit: Option<&Path>) -> Result<Loaded> {
    let path = match explicit {
      Some(path) => path.to_path_buf(),
      None => match env::var_os("CLDR_CONFIG").filter(|v| !v.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => config_path()?,
      },
    };
    if !path.exists() {
      return Ok(Loaded {
        path,
        exists: false,
        config: Config::default(),
      });
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let config = Config::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Loaded {
      path,
      exists: true,
      config,
    })
  }

  pub fn zone(&self) -> Result<TimeZone> {
    match &self.journal.timezone {
      Some(name) => Ok(TimeZone::get(name).map_err(|e| format!("journal.timezone {name}: {e}"))?),
      None => Ok(TimeZone::system()),
    }
  }

  /// The default zone and the dated changes.
  pub fn zones(&self) -> Result<Zones> {
    let mut changes = Vec::with_capacity(self.journal.zones.len());
    for z in &self.journal.zones {
      let zone = TimeZone::get(&z.zone).map_err(|e| format!("journal.zones {}: {e}", z.zone))?;
      changes.push((z.from, zone));
    }
    Zones::new(self.zone()?, changes)
  }

  pub fn journal_root(&self) -> Result<Option<PathBuf>> {
    self.journal.root.as_deref().map(expand).transpose()
  }

  pub fn store_path(&self) -> Result<PathBuf> {
    match &self.calendar.store {
      Some(path) => expand(path),
      None => Ok(home()?.join(crate::calendar::busycal::STORE)),
    }
  }
}

pub fn config_path() -> Result<PathBuf> {
  Ok(xdg_path("XDG_CONFIG_HOME", ".config")?.join("config.toml"))
}

/// The app's directory under an XDG base. Empty or relative overrides are ignored, as XDG requires.
pub fn xdg_path(variable: &str, fallback: &str) -> Result<PathBuf> {
  let base = env::var_os(variable)
    .map(PathBuf::from)
    .filter(|p| p.is_absolute());
  let base = match base {
    Some(base) => base,
    None => home()?.join(fallback),
  };
  Ok(base.join(APP))
}

pub fn home() -> Result<PathBuf> {
  env::var_os("HOME")
    .map(PathBuf::from)
    .filter(|p| p.is_absolute())
    .ok_or_else(|| "HOME is not set to an absolute path".into())
}

/// Expand a leading `~/`; anything else is taken as written.
pub fn expand(path: &str) -> Result<PathBuf> {
  match path.strip_prefix("~/") {
    Some(rest) => Ok(home()?.join(rest)),
    None if path == "~" => home(),
    None => Ok(PathBuf::from(path)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_empty_config_takes_every_default() -> Result<()> {
    let config = Config::parse("")?;
    let defaults = Rules::default();
    assert_eq!(config.calendar.source, default_source());
    assert_eq!(config.rules.min_weekday_hours, defaults.min_weekday_hours);
    assert!(config.calendar.include.is_empty());
    Ok(())
  }

  #[test]
  fn unknown_keys_are_refused() {
    assert!(Config::parse("[journal]\nrooot = \"x\"\n").is_err());
  }

  #[test]
  fn the_example_config_parses() -> Result<()> {
    let text = include_str!("../examples/config.toml");
    let config = Config::parse(text)?;
    assert!(config.journal.root.is_some());
    assert!(!config.tags.is_empty());
    assert!(!config.journal.zones.is_empty());
    config.zones()?;
    Ok(())
  }

  #[test]
  fn an_unknown_zone_is_refused() -> Result<()> {
    let config =
      Config::parse("[[journal.zones]]\nfrom = \"2026-01-01\"\nzone = \"Nowhere/Atall\"\n")?;
    assert!(config.zones().is_err());
    Ok(())
  }
}
