# Cldr

A re-runnable renderer for a plain-text time journal. You write one Markdown file a month, the Day Notes, with a small time log table under each day. Cldr reads it, and your calendar for any day you did not write up, and derives everything else: week and month summaries inside the Day Notes, a JSON document per month, a year's hours report, and the generated parts of a Daily Briefing. Rust; no language model anywhere in the pipeline; rendering twice changes nothing.

The design is [TN0001](docs/notes/tn0001-cldr-design-note.md); how it was built and what is open is [TN0002](docs/notes/tn0002-cldr-build-plan.md).

## Install

```sh
brew install matthewsinclair/cldr/cldr
```

The formula installs the signed and notarised cldr for macOS on Apple silicon. From a checkout: `bin/devbin build`, which runs `cargo build --release --locked`.

## Set up

```sh
mkdir -p ~/.config/cldr
cp "$(brew --prefix)/share/cldr/config.toml" ~/.config/cldr/config.toml   # or examples/config.toml
$EDITOR ~/.config/cldr/config.toml
cldr doctor
```

The config holds everything personal: where the journal is, the zones you were in, how tags are spelt, what is not work, what bills, the month templates and links, and the briefing's blocks. See [examples/config.toml](examples/config.toml). `CLDR_CONFIG` or `--config PATH` points elsewhere.

The calendar comes from BusyCal's local store, which holds every account BusyCal syncs, and is only ever read. On macOS the terminal needs Full Disk Access to read it; `cldr doctor` says so when it is missing.

## The journal

```text
journal/
  20261101-20261130-day-notes.md        you write this; cldr adds its sections
  20261101-20261130-data.json           cldr render
  20261101-20261130-link-dump.md        yours
  20261101-20261130-daily-briefing.md   cldr briefing writes its gen blocks; the rest is yours
  2026-hours-ytd.md                     cldr render year
  day_notes.md -> 20261101-20261130-day-notes.md
  2026/202610/...                       earlier months, archived by cldr month new
```

A day in the Day Notes:

```markdown
#### 20261102 Monday

#### Time Log

| Project | Time        | Hours | Bill | What         |
|---------|:-----------:|------:|-----:|--------------|
| @acme   | 07:00-09:00 |   2.0 |  0.5 | Travel       |
| @acme   | 09:00-12:30 |   3.5 |      | Workshop     |
|         |             |   5.5 |      |              |
```

Days and weeks run newest first; a week heading is `#### YYYYMMDD (Week NN)`. The `Bill` column is optional. Cldr writes only between its own `<!-- cldr:… -->` markers and leaves every other byte as you wrote it.

## Commands

```sh
cldr render week               # last week's summary, and the month's JSON
cldr render month 2026-11      # every begun week of the month, the grid, the JSON
cldr render year 2026          # 2026-hours-ytd.md
cldr briefing                  # the Daily Briefing (from the cutover hour, tomorrow's)
cldr check month               # totals and every flag; writes nothing
cldr data month                # the structured data as JSON
cldr month new                 # start the month: templates, links, archive the last one
cldr export week --stdout      # what the calendar reader sees, as CSV
cldr doctor                    # config, journal, zones, calendar access
```

`cldr help` lists every option. Writes go through a temporary file, and `--dry-run` shows a write before it happens.

## Developing

`AGENTS.md` maps the source and lists the invariants. `cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` must pass; CI runs them on macOS and Linux. To release, from a clean `main` that matches GitHub, with the tap checked out at `../Brew/homebrew-cldr`: `tools/release --dry-run X.Y.Z`, then `tools/release X.Y.Z`.

## Licence

MIT.
