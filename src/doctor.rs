//! Checks a setup and says what to fix.

use crate::calendar::busycal::BusyCal;
use crate::config::Loaded;

pub struct Check {
  pub ok: bool,
  pub what: String,
  pub detail: String,
}

pub fn run(loaded: &Loaded) -> Vec<Check> {
  let mut checks = Vec::new();
  let mut push = |ok: bool, what: &str, detail: String| {
    checks.push(Check {
      ok,
      what: what.into(),
      detail,
    })
  };
  let config = &loaded.config;

  push(
    loaded.exists,
    "config",
    if loaded.exists {
      loaded.path.display().to_string()
    } else {
      format!(
        "{} not found; copy examples/config.toml there",
        loaded.path.display()
      )
    },
  );

  match config.journal_root() {
    Ok(Some(root)) => push(root.is_dir(), "journal", root.display().to_string()),
    Ok(None) => push(false, "journal", "journal.root is not set".into()),
    Err(e) => push(false, "journal", e.to_string()),
  }

  let zone = config.zones();
  match &zone {
    Ok(zone) => push(true, "zone", describe(zone)),
    Err(e) => push(false, "zone", e.to_string()),
  }

  match (config.store_path(), zone) {
    (Ok(store), Ok(zone)) => {
      if !store.exists() && std::fs::metadata(store.parent().unwrap_or(&store)).is_err() {
        push(
          false,
          "calendar",
          store_denied(&store.display().to_string()),
        );
      } else {
        match BusyCal::open(&store, zone, &config.calendar.me).and_then(|c| c.calendar_count()) {
          Ok(n) => push(
            true,
            "calendar",
            format!("{} ({n} calendars)", store.display()),
          ),
          Err(e) => push(
            false,
            "calendar",
            format!("{e}; {}", store_denied(&store.display().to_string())),
          ),
        }
      }
    }
    (Err(e), _) | (_, Err(e)) => push(false, "calendar", e.to_string()),
  }
  checks
}

fn describe(zones: &crate::zones::Zones) -> String {
  let name = |z: &jiff::tz::TimeZone| z.iana_name().unwrap_or("system").to_owned();
  let changes = zones.changes();
  if changes.is_empty() {
    return name(zones.default_zone());
  }
  let dated: Vec<String> = changes
    .iter()
    .map(|(from, z)| format!("{} from {from}", name(z)))
    .collect();
  format!("{}, then {}", name(zones.default_zone()), dated.join(", "))
}

fn store_denied(path: &str) -> String {
  format!(
    "cannot read {path}. On macOS the store is in another app's data folder: give this terminal Full Disk Access \
     (System Settings > Privacy & Security > Full Disk Access), then reopen it. An OS update can revoke this."
  )
}
