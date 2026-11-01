# TN0001: Cldr design note

Status: design settled, ready to build. Nothing here is built yet.

## The problem

A plain-text time journal keeps one folder of Markdown per month: Day Notes (a time log per day, written by hand), an Activity Log (week tables, a weekly summary and a month grid), a Link Dump, and a Daily Briefing. The tooling that maintains it grew around a calendar it could not read: the week's calendar was printed to PDF, turned into CSV by OCR, parsed into an analysis text file, and a language model read that file with the Day Notes and wrote the Activity Log. Every hand-off in that chain is a place to lose or invent data. In one checked week the OCR dropped an event and silently corrected two titles. The wrap is a one-shot ritual, its rules live in prose, and nothing can be re-run to check it.

Two things change the picture:

- The calendar can be read directly. BusyCal syncs every account (Google, Microsoft 365 and others) into one local SQLite store, readable read-only.
- The Day Notes time log already records every hour by tag, time and day, so the Activity Log need not copy its rows.

So everything in the Activity Log, the month grid and the hour totals is a function of two sources. A function can be re-run at any time, diffed, and tested.

## Goals

- One Rust binary, `cldr`, that reads the calendar and the Day Notes and renders every derived file from them.
- Re-runnable: rendering the same inputs gives the same output; rendering twice changes nothing.
- No language model anywhere in the pipeline. Every rule is code with a test.
- Nothing personal in the repository. Names, paths, calendars, tags and templates live in the user's config, outside the repo.
- Hand-written content is never touched: Day Notes, Link Dump, and the hand-written parts of the briefing.

Out of scope: the content of the Link Dump and the TWIL notes (they move with their month but are never rewritten), invoicing and timesheets (a later consumer of the same model).

## Sources of truth

1. **Day Notes time log** (authoritative). Each day's `Time Log` table: project tag, time range, hours, what. Where a day has rows, they are the record of that day.
2. **Calendar** (fallback and cross-check). Events from the configured calendars for the same range, declined and cancelled events excluded.

A day with Day Notes rows takes its hours from them. A day without rows takes them from the calendar, by the rules below, and is flagged as calendar-only. Where both exist and differ by more than a configured threshold, the day is flagged; the Day Notes still win. Flags go to the terminal and to `cldr check`, never into the journal.

Decision D1 (settled): the Day Notes are the record and the calendar is the fallback. This reverses the old prompt's order, which made the calendar primary and used Day Notes only on double-booked days. In practice the Day Notes have been treated as the record (meetings overrun their invitations; the calendar shows the invitation).

## Model

- **Entry**: date, start, end, hours, tag, what, source (`notes` or `calendar`).
- **Day**: the entries for one date, a total, and flags.
- **Week**: Monday to Sunday, ISO week number. A week spanning two months belongs to the earlier month's Activity Log.
- **Month**: the calendar month; its grid covers every day of its own month, including the days of a spanning week whose summary sits in the previous month.

## Rules

Each of these is a function with tests. Values in brackets come from config.

- **Tags**: Day Notes tags are matched case-insensitively and normalised to the configured spelling (`@acme` and `@Acme` are one tag).
- **Not work**: entries with an excluded tag or a title matching an excluded prefix (eg `[GYM]`) count zero hours [`rules.exclude`].
- **Holds**: calendar blocks matching a hold prefix (eg `Travel:`) count zero hours [`rules.zero_hours`].
- **Weekend tags**: tags that may not appear on Saturday or Sunday are flagged, not moved [`rules.weekday_only`].
- **Midnight**: a session that runs past midnight is counted in full on the day it started. Contiguous blocks either side of midnight are one session; a block starting at 00:00 after a gap belongs to its own day. This holds across week and month boundaries.
- **Breaks**: not deducted; a session counts its span.
- **Calendar overlaps**: on a calendar-only day, overlapping blocks count once (net distinct time), and the overlap is reported.
- **Flags**: a weekday under the minimum [`rules.min_weekday_hours`]; a Day Notes row with a time range but no hours, or hours that disagree with the range; a gap between sources over the threshold [`rules.max_gap_hours`].
- **Grid codes**: whole hours, floored: `0` to `9`, then `A` for 10 up to `K` for 20.

## Outputs

- **Activity Log**: one section per week, holding the weekly summary (hours per tag per day, with totals), and the month grid. `cldr` owns these regions and nothing else in the file. The Activity Log no longer copies the Day Notes rows into per-day tables: a day's rows are written in one place only, the Day Notes.
- **Hours**: per-tag grids by week, month and year, computed from the model, never parsed back out of the Activity Log.
- **Daily Briefing**: the generated blocks (today, tomorrow, the week, the month), as today. The hand-written sections stay hand-written.
- **Calendar CSV export**: `Date,From,To,Title,Modified` for one week or month, for the transition only, so the existing week wrap can read the calendar without PDFs or OCR. Retired once rendering replaces it.

### Managed regions

`cldr` writes only between markers it owns, eg `<!-- cldr:week 2026-W44 -->` and `<!-- /cldr:week -->`. Everything outside the markers is preserved byte for byte. A file with no markers yet gets them on first render; existing hand-rendered weeks are adopted by `cldr adopt`, which wraps them in markers after a clean `check`.

## Commands

```text
cldr render week [DATE]        render the week holding DATE (default last week)
cldr render month [YYYY-MM]    render every week of the month and its grid
cldr check week|month [...]    print the diff and the flags; write nothing
cldr month new [YYYY-MM]       create the month's files and archive the previous month
cldr briefing [DATE]           refresh the briefing's generated blocks
cldr hours week|month|year     per-tag hour grids
cldr export week|month [DATE]  calendar CSV, for the transition
cldr doctor                    config, paths, calendar access, journal layout
cldr paths                     resolved XDG paths
```

`render` is safe to run at any time: after a Day Note changes, mid-week, or long after the week ends.

## Month lifecycle

`cldr month new` replaces the month wrap:

1. Create the new month's Activity Log, Day Notes, Link Dump and Daily Briefing from the configured templates. Never overwrite an existing file.
2. Add the spanning-week marker to the new Activity Log when the first week belongs to the previous month.
3. Render the previous month one last time and run `check`; stop if it reports a diff the user has not seen.
4. Move the previous month's files into `YYYY/YYYYMM/`. In a git work tree this is `git mv`, so history follows the files; otherwise a plain move.
5. Repoint the current-briefing symlink at the new month's briefing. If a sync service has turned the link into a plain file, compare it with the month's file first, stop if it holds anything the month's file lacks, and otherwise replace it with the link.

Nothing in a moved file is rewritten. The Link Dump in particular moves untouched.

## Calendar access

The source is BusyCal's local store: the union of the user's calendar accounts (Google, Microsoft 365 and any other BusyCal syncs), so one reader covers every account with no logins or tokens of its own. Config selects accounts and calendars by name. It is opened read-only (`?mode=ro`); `cldr` never writes to it. The reader sits behind a `CalendarSource` trait so that another source (an ICS export, EventKit) can be added without touching the rules. On macOS the store sits in another app's group container, which needs Full Disk Access for the terminal running `cldr`; an OS update can revoke it. `cldr doctor` checks access and names the setting when it is missing. Times are read in the event's zone and rendered in the configured zone.

## Configuration

TOML at `$XDG_CONFIG_HOME/cldr/config.toml` (default `~/.config/cldr/config.toml`); `CLDR_CONFIG` or `--config PATH` overrides it. State and cache follow `XDG_STATE_HOME` and `XDG_CACHE_HOME`. Empty or relative XDG overrides are ignored, as in Ccusage.

```toml
[journal]
root = "~/Documents/Journal"
author = "Your Name"            # verblock author in new files
timezone = "Europe/London"
templates = "~/.config/cldr/templates"

[calendar]
source = "busycal"
include = ["you@example.com", "you@work.example"]   # account or calendar name substrings
exclude = ["Holidays"]

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

Laid out after Ccusage: `Cargo.toml` (edition 2024, `unsafe_code = "forbid"`, clippy `unwrap_used` and `expect_used` denied), `rustfmt.toml` with two-space tabs, `src/` by concern (`config`, `calendar`, `notes`, `model`, `rules`, `render`, `month`, `briefing`, `cli`, `doctor`), `tests/` with CLI tests over generated journals, `.github/workflows/` building macOS and Linux, a Homebrew formula under `packaging/`, MIT licence. Release tooling is written in Rust or shell; the repo ships no Python.

## Privacy

The repository is public. It holds no names, addresses, client or project names, calendar names, tags or paths from any real journal. Fixtures are generated by the tests from synthetic data. The example config and templates use placeholders. Anything personal is in the user's config directory.

## Testing and acceptance

- **Rules**: unit tests per rule, including the midnight cases at week and month boundaries.
- **Rendering**: round-trip tests over generated journals: render, render again, no change; hand content outside markers unchanged.
- **Acceptance: the whole year** (local, never committed). Run `cldr check` over every week and month of a real journal's current year and compare with the Activity Logs already written. Every difference is classified: a `cldr` bug (fixed), an error or inconsistency in the old log (listed for the user, and corrected on their word), or a change the rules intend (eg the overnight convention applied where the old wrap missed it). The bar is not "the same numbers": it is a year that renders the same way every time, with every difference from the old logs explained. Re-running over the year after any change to the rules is the regression test.
- **Consistency as a product**: the year run also reports what makes the journal harder to work with: tags spelt several ways, Day Notes rows without hours, days with neither notes nor calendar, weeks whose markers are missing. `cldr check` prints these as a list the user can work through.
- **Tests**: expected values are derived from the input the test generates, never from a count written by hand.
- **Parallel run**: until the year run is clean, `cldr check` runs beside the old wrap.

## Migration

1. `cldr export week|month`: the CSV from the store; the PDFs and OCR stop.
2. `cldr render` and `check` beside the old week wrap. Re-rendering a past week replaces its old day tables with the summary; git keeps the old form.
3. `cldr month new` with the archive move; the month wrap and its prompt are retired.
4. `cldr hours` and `cldr briefing`; the old scripts and the Python helpers are retired.
5. Remove the old `bin/` scripts, prompts and `_processing/` files from the journal.

## Settled

- D1: the Day Notes are the record; the calendar is the fallback and the cross-check.
- The old `timelog` and `convert_day_notes` commands are not carried over.
- The Activity Log drops its per-day tables: weekly summary and month grid only.
- The current-briefing file stays a symlink, repointed each month.
- BusyCal's store is the calendar source, covering every synced account; no direct Google or Microsoft readers in the first version.
