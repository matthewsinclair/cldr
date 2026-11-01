# TN0001: Cldr design note

Status: design settled (D1, D2, D3); building (step 2 done). Build plan: [TN0002](tn0002-cldr-build-plan.md).

## The problem

A plain-text time journal keeps one folder of Markdown per month: Day Notes (a time log per day, written by hand), an Activity Log (week tables, a weekly summary and a month grid), a Link Dump, and a Daily Briefing. The tooling that maintains it grew around a calendar it could not read: the week's calendar was printed to PDF, turned into CSV by OCR, parsed into an analysis text file, and a language model read that file with the Day Notes and wrote the Activity Log. Every hand-off in that chain is a place to lose or invent data. In one checked week the OCR dropped an event and silently corrected two titles. The wrap is a one-shot ritual, its rules live in prose, and nothing can be re-run to check it. Turning a month over is enough work that it gets put off.

Two things change the picture:

- The calendar can be read directly. BusyCal syncs every account (Google, Microsoft 365 and others) into one local SQLite store, readable read-only.
- The Day Notes time log already records every hour by tag, time and day.

So every summary, grid and total is a function of two sources. A function can be re-run at any time, diffed, and tested.

## Goals

- One Rust binary, `cldr`, that reads the calendar and the Day Notes and derives everything else from them.
- One hand-written file per month: the Day Notes. `cldr` instruments it with generated sections and emits structured data; there is no separate Activity Log.
- Re-runnable: the same inputs give the same output; rendering twice changes nothing.
- No language model anywhere in the pipeline. Every rule is code with a test.
- Nothing personal in the repository. Names, paths, calendars, tags and templates live in the user's config, outside the repo.
- Hand-written content is never touched: the Day Notes' own text and tables, the Link Dump, and the hand-written parts of the briefing.

Out of scope: the content of the Link Dump and other notes in the Day Notes (they move with their month but are never rewritten), invoicing and timesheets (later consumers of the structured data).

## Sources of truth

1. **Day Notes time log** (authoritative). Each day's `Time Log` table: project tag, time range, hours, what. Where a day has rows, they are the record of that day.
2. **Calendar** (fallback and cross-check). Events from the configured accounts and calendars for the same range, declined and cancelled events excluded.

A day with Day Notes rows takes its hours from them. A day without rows takes them from the calendar, by the rules below, and is flagged as calendar-only. Where both exist and differ by more than a configured threshold, the day is flagged; the Day Notes still win. Flags go to the terminal, to `cldr check` and to the structured data, never into the hand-written text.

## Model

- **Entry**: date, start, end, hours, tag, what, source (`notes` or `calendar`).
- **Day**: the entries for one date, a total, and flags.
- **Week**: Monday to Sunday, ISO week number. A week spanning two months is summarised in the earlier month's Day Notes.
- **Month**: the calendar month. Its grid covers every day of its own month, including the days of a spanning week whose summary sits in the previous month.

## Rules

Each of these is a function with tests. Values in brackets come from config.

- **Tags**: tags are matched case-insensitively and normalised to the configured spelling (`@acme` and `@Acme` are one tag).
- **Not work**: entries with an excluded tag or a title matching an excluded prefix (eg `[GYM]`) count zero hours [`rules.exclude`].
- **Holds**: calendar blocks matching a hold prefix (eg `Travel:`) count zero hours [`rules.zero_hours`].
- **Weekend tags**: tags that may not appear on Saturday or Sunday are flagged, not moved [`rules.weekday_only`].
- **Midnight**: a session that runs past midnight is counted in full on the day it started. Contiguous blocks either side of midnight are one session; a block starting at 00:00 after a gap belongs to its own day. This holds across week and month boundaries.
- **Breaks**: not deducted; a session counts its span.
- **Calendar overlaps**: on a calendar-only day, overlapping blocks count once (net distinct time), and the overlap is reported.
- **Flags**: a weekday under the minimum [`rules.min_weekday_hours`]; a Day Notes row with a time range but no hours, or hours that disagree with the range; a gap between sources over the threshold [`rules.max_gap_hours`]; a tag spelt more than one way; Day Notes rows whose ranges overlap (the shared time is counted twice).
- **Grid codes**: whole hours, floored: `0` to `9`, then `A` for 10 up to `K` for 20. Over 20 is `+`.

Settled while building step 2:

- **Unconfigured tags** are lower case; `[tags]` maps any case of a key, and of its value, to the value's spelling. A tag written without `@` gains it. The spelling flag looks at the Day Notes only.
- **Written hours win** over a row's time range; a difference over a quarter of an hour (`HOURS_TOLERANCE`) is flagged. A row with a range and no hours takes the range, flagged; a row with neither counts nothing, flagged.
- **Total rows** are not used; a total that differs from the sum of its rows is flagged.
- **Calendar time** counts only when the title starts with a bracketed tag (`[Acme] Workshop` is `@acme`); untagged events are listed, not counted. The calendar speaks only for days up to today: planned time is not worked time.
- **A day written in two files** (a spanning week's days often are) is read from the file whose name covers the date; a different copy elsewhere is flagged.
- **Billable hours** (hv, option 2): `[bill]` gives each tag a fraction from 0 to 1 (unlisted tags bill nothing), and a Time Log table may add a `Bill` column after Hours (`| Project | Time | Hours | Bill | What |`) to override a row, eg `0.5` for travel. Tables without the column stay valid. Billable hours are hours times the fraction, for counted time only; every total carries `billable` and `billable_by_tag` beside `hours` and `by_tag`. Turning billable hours into days for a day-rate client is the timesheet's job, not cldr's.
- **Weeks are whole**: `data month` reads the whole weeks at either end, so a spanning week's summary is complete; the month's own totals and grid cover its own days.

## Outputs

- **Generated sections in the Day Notes**: the month's grid and per-tag totals at the top of the file; under each week's heading, that week's summary (hours per tag per day, with totals). These are the only parts of the file `cldr` writes.
- **Structured data**: one JSON document per month beside its Day Notes, written on every render: entries with their source, days, weeks, per-tag totals and flags. The same document goes to stdout with `--json`, for a week, a month or a year. This is the interface for anything else that wants the hours (invoices, timesheets, year-to-date figures); nothing parses the Markdown back.
- **Daily Briefing**: the generated blocks (today, tomorrow, the week, the month), as today. The hand-written sections stay hand-written.
- **Calendar CSV export**: `Date,From,To,Title,Modified` for one week or month, for the transition only, so the existing week wrap can read the calendar without PDFs or OCR. Retired once rendering replaces it.

### Generated sections

`cldr` writes only between markers it owns, eg `<!-- cldr:week 2026-W44 -->` and `<!-- /cldr:week -->`, and `<!-- cldr:month 2026-10 -->` and `<!-- /cldr:month -->`. Everything outside the markers is preserved byte for byte. The Day Notes already carry a heading per week (`#### 20261026 (Week 44)`); the week's section goes directly under it. A week with no heading yet gets one, at the top, in the file's own newest-first order.

## Commands

```text
cldr render week [DATE]        the week's generated section and the month's JSON (default last week)
cldr render month [YYYY-MM]    every week of the month, the grid, and the JSON
cldr check week|month|year     print the diff and the flags; write nothing
cldr data week|month|year      the structured data as JSON on stdout
cldr month new [YYYY-MM]       create the month's files and archive the previous month
cldr briefing [DATE]           refresh the briefing's generated blocks
cldr export week|month [DATE]  calendar CSV, for the transition
cldr doctor                    config, paths, calendar access, journal layout
cldr paths                     resolved XDG paths
```

`render` is safe to run at any time: after a Day Note changes, mid-week, or long after the week ends. Week and month wraps become `render`.

## Month lifecycle

`cldr month new` replaces the month wrap:

1. Render the previous month one last time and run `check`; stop if it reports a diff the user has not seen.
2. Create the new month's Day Notes, Link Dump and Daily Briefing from the configured templates. Never overwrite an existing file.
3. Note the spanning week at the top of the new Day Notes when the first week is summarised in the previous month.
4. Move the previous month's files (Day Notes, its JSON, Link Dump, Daily Briefing, and an Activity Log if one exists) into `YYYY/YYYYMM/`. In a git work tree this is `git mv`, so history follows the files; otherwise a plain move.
5. Repoint the current-briefing symlink at the new month's briefing. If a sync service has turned the link into a plain file, compare it with the month's file first, stop if it holds anything the month's file lacks, and otherwise replace it with the link.

Nothing in a moved file is rewritten. The Link Dump in particular moves untouched.

## Calendar access

The source is BusyCal's local store: the union of the user's calendar accounts (Google, Microsoft 365 and any other BusyCal syncs), so one reader covers every account with no logins or tokens of its own. Config selects accounts and calendars by name. It is opened read-only (`?mode=ro`); `cldr` never writes to it. The reader sits behind a `CalendarSource` trait so that another source can be added without touching the rules. BusyCal is only a viewer: the real sources are the user's accounts, and reading BusyCal's private cache ties `cldr` to that app's undocumented schema. The second source is EventKit, the macOS calendar layer those same accounts sync into, which also expands repeating events itself. Its bindings need `unsafe`; that is confined to one module with the crate otherwise forbidding it. On macOS the store sits in another app's group container, which needs Full Disk Access for the terminal running `cldr`; an OS update can revoke it. `cldr doctor` checks access and names the setting when it is missing.

The store's schema is undocumented. What the reader relies on: events are `ZCOMMONDATA` rows (entity 5 an event, 6 a changed occurrence of a repeating event, 8 a calendar); times are stored as seconds since 2001 with a zone name; all-day events sit at noon UTC with an exclusive end; titles and calendar names are keyed archives; repeating events carry a rule (frequency, interval, count, until, by-day, by-month-day, by-month, set positions, exception dates) that the reader expands; the same invitation on two calendars shares a UID and is reported once; event status 7 is cancelled, attendee status 9 is declined.

## Configuration

TOML at `$XDG_CONFIG_HOME/cldr/config.toml` (default `~/.config/cldr/config.toml`); `CLDR_CONFIG` or `--config PATH` overrides it. State and cache follow `XDG_STATE_HOME` and `XDG_CACHE_HOME`. Empty or relative XDG overrides are ignored, as in Ccusage.

```toml
[journal]
root = "~/Documents/Journal"
author = "Your Name"            # verblock author in new files
timezone = "Europe/London"      # the default zone; the machine's zone when absent
templates = "~/.config/cldr/templates"

# Where you were: from each date on, the journal's wall clock is in this zone.
[[journal.zones]]
from = "2026-09-27"
zone = "Pacific/Auckland"

[[journal.zones]]
from = "2026-12-20"
zone = "Europe/London"

[calendar]
source = "busycal"
include = ["you@example.com", "you@work.example"]   # account or calendar name substrings
exclude = ["Holidays"]
me = ["you@example.com", "you@work.example"]        # your addresses, to read your replies

[tags]
"@acme" = "@Acme"
"@self" = "@Self"

[rules]
exclude = ["[GYM]", "@personal"]
zero_hours = ["Travel:"]
weekday_only = ["@Acme"]
min_weekday_hours = 8.0
max_gap_hours = 1.0
```

Templates live in the config directory, not the repo, because they carry the author's name and layout. The repo ships neutral example templates under `examples/`.

## Repository

Laid out after Ccusage: `Cargo.toml` (edition 2024, `unsafe_code = "forbid"`, clippy `unwrap_used` and `expect_used` denied), `rustfmt.toml` with two-space tabs, `src/` by concern, `tests/` with CLI tests over generated journals, `.github/workflows/` building macOS and Linux, a Homebrew formula under `packaging/`, MIT licence. Release tooling is written in Rust or shell; the repo ships no Python.

## Privacy

The repository is public. It holds no names, addresses, client or project names, calendar names, tags or paths from any real journal. Fixtures are generated by the tests from synthetic data. The example config and templates use placeholders. Anything personal is in the user's config directory.

## Testing and acceptance

- **Rules**: unit tests per rule, including the midnight cases at week and month boundaries.
- **Rendering**: round-trip tests over generated journals: render, render again, no change; text outside markers unchanged.
- **Calendar reader**: tests over a synthetic store built by the tests in the same schema, covering each recurrence form, exceptions, moved occurrences, all-day spans, merged invitations, declined and cancelled events.
- **Acceptance: the whole year** (local, never committed). Run `cldr check year` over a real journal and compare with the Activity Logs the old tooling wrote. Every difference is classified: a `cldr` bug (fixed), an error or inconsistency in the old log (listed for the user, and corrected on their word), or a change the rules intend (eg the overnight convention applied where the old wrap missed it). The bar is not "the same numbers": it is a year that renders the same way every time, with every difference from the old logs explained. Re-running over the year after any change to the rules is the regression test.
- **Consistency as a product**: the year run also lists what makes the journal harder to work with: tags spelt several ways, Day Notes rows without hours, days with neither notes nor calendar, weeks without headings.
- **Tests**: expected values are derived from the input the test generates, never from a count written by hand.

## Migration

The old wraps are not run again. The first month `cldr` turns over is the first month it is ready for; until then the Day Notes carry on as they are, and nothing is lost by waiting, because everything `cldr` writes can be derived later.

1. `cldr export week|month`: the CSV from the store; the PDFs and OCR stop.
2. `cldr data` and `check year`, until the year's differences against the old Activity Logs are all explained.
3. `cldr render` instruments the Day Notes. Past Activity Logs stay where they are, unchanged, as history.
4. `cldr month new` turns over every month still waiting, oldest first, with the archive move.
5. `cldr briefing`; the old scripts, prompts, Python helpers and `_processing/` files are removed from the journal.

## Settled

- D1: the Day Notes are the record; the calendar is the fallback and the cross-check.
- D2: one hand-written file per month. The Day Notes carry generated week and month sections; a JSON document per month carries the structured data; there is no Activity Log.
- The old `timelog` and `convert_day_notes` commands are not carried over.
- The current-briefing file stays a symlink, repointed each month.
- BusyCal's store is the first calendar source. EventKit (the macOS calendar layer) is the second, chosen in config, and becomes the default once it agrees with BusyCal across a year; no direct Google or Microsoft readers.
- D3: zones are dated. The journal's wall-clock zone follows where the user was, from a dated list in the config.
- The old week and month wraps are not run again; months wait for `cldr month new`.
- File names: a date or date range, then a kebab-case kind: `20261101-20261130-day-notes.md`, `20261026-20261101-w44-calendar.csv`. The kind is what `[month.templates]` and `[month.links]` name. `src/names.rs` is the one place names are made and read.
