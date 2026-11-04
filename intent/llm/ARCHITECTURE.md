---
verblock: "2026-11-04T05:24:12Z:v0.1: matts - The module map and the invariants, moved from AGENTS.md"
---

# ARCHITECTURE.md

How Cldr is shaped, and why, for anyone, human or agent, changing this code. `README.md` is the user's view; `docs/notes/tn0001-cldr-design-note.md` is the design and its rules; `docs/notes/tn0002-cldr-build-plan.md` is the build record and the open work. The rules for changes, and how to work on and release Cldr, are in `intent/llm/RULES.md`.

## The source

| Module        | What it does                                                                                                    |
| ------------- | --------------------------------------------------------------------------------------------------------------- |
| `cli.rs`      | Argument parsing and the commands; thin: each command builds inputs and calls the modules below                 |
| `config.rs`   | The TOML config, XDG paths, `~` expansion; unknown keys refused                                                 |
| `zones.rs`    | Dated zones: the zone of each date and each instant                                                             |
| `period.rs`   | Week, month and year spans                                                                                      |
| `names.rs`    | Every journal file name, made and read in one place                                                             |
| `notes.rs`    | The lenient Day Notes parser: days, weeks, Time Log rows with their lines, and every format problem             |
| `journal.rs`  | Finding the journal's files by kind                                                                             |
| `model.rs`    | The rules: entries, days, weeks, months, totals, flags; `Rulebook` holds tag spelling, exclusions and billing   |
| `render.rs`   | The generated sections in the Day Notes, and the year report                                                    |
| `briefing.rs` | The Daily Briefing's gen blocks, rolling a day under Earlier, running configured blocks                         |
| `month.rs`    | `month new`: the plan, templates, links, the archive move                                                       |
| `check.rs`    | The check report, and the comparison with old Activity Logs                                                     |
| `legacy.rs`   | Reading old Activity Logs, for that comparison only                                                             |
| `export.rs`   | Calendar CSV                                                                                                    |
| `doctor.rs`   | Setup checks                                                                                                    |
| `calendar/`   | `CalendarSource` and `Event`; `busycal.rs` the store reader, `recur.rs` recurrence, `archive.rs` keyed archives |

## Invariants

- **Hand-written text is never changed.** cldr writes only between its markers (`<!-- cldr:... -->`, `<!-- gen:... -->`) and the files it owns whole (`data.json`, `YYYY-hours-ytd.md`). A change to `render.rs` or `briefing.rs` keeps the round-trip tests: render twice, no change; the text outside markers intact.
- **The calendar store is only read.** It is opened read-only; never write to it.
- **The Day Notes are the record.** The calendar fills a day only when it has no rows, and only up to today.
- **One place for each thing**: file names in `names.rs`, tag and work rules in `Rulebook`, totals in `Totals`, zones in `Zones`. Add to them; do not reimplement them elsewhere.
- **Pure core, thin shell**: the model, render and briefing functions take their inputs and return text or data; only `cli.rs` and `month::apply` touch the disk, and writes go through a temporary file.
