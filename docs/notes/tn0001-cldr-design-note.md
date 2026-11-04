# TN0001: Cldr design note

Status: built (v0.2.0) and in daily use. This note is the design as it stands; [TN0002](tn0002-cldr-build-plan.md) records how it was built and what is still open.

## The problem

A plain-text time journal keeps one folder of Markdown per month: Day Notes (a time log per day, written by hand), a Link Dump and a Daily Briefing. Its earlier tooling grew around a calendar it could not read: the week's calendar was printed to PDF, turned into CSV by OCR, and a language model read that with the Day Notes and wrote a monthly Activity Log. Every hand-off in that chain could lose or invent data, its rules lived in prose, nothing could be re-run to check it, and turning a month over was enough work that it got put off.

Two facts change the picture: the calendar can be read directly (BusyCal syncs every account into one local SQLite store), and the Day Notes already record every hour by tag, time and day. So every summary, grid and total is a function of two sources, and a function can be re-run, diffed and tested.

## Goals

- One Rust binary, `cldr`, that reads the Day Notes and the calendar and derives everything else.
- One hand-written file per month, the Day Notes. `cldr` writes generated sections into it and structured data beside it; there is no separate Activity Log.
- Re-runnable: the same inputs give the same output; rendering twice changes nothing.
- No language model anywhere in the pipeline. Every rule is code with a test.
- Nothing personal in the repository: names, paths, calendars, tags and templates live in the user's config.
- Hand-written content is never touched: the Day Notes' own text and tables, the Link Dump, and the hand-written parts of the briefing.

Out of scope: rewriting notes or links, and invoicing and timesheets, which consume the structured data.

## Sources of truth

1. **The Day Notes time log** (the record). Where a day has Time Log rows, they are that day.
2. **The calendar** (fallback and cross-check). A day without rows takes its hours from the calendar, by the rules below, and is flagged as calendar-only. Where both exist and differ by more than a threshold, the day is flagged; the Day Notes still win.

Flags go to the terminal, `cldr check` and the structured data, never into hand-written text.

## The Day Notes format

- Days and weeks run newest first. A day heading is `#### YYYYMMDD Weekday`; a week heading is `#### YYYYMMDD (Week NN)`, dated by its Monday.
- Under a day, a `Time Log` heading and a table whose header names the columns: Project, Time, Hours, What, with an optional `Bill` after Hours. Columns are found by the header, so four- and five-column tables both read.
- The row with no project and some hours is the day's total, for the reader only.
- Reading is lenient: a time written `20:00:02:00` or `09;00-10:00`, hours with a comma, a stray cell, a wrong weekday, a repeated day are all read where possible and reported with their line.

## Model

- **Entry**: date, start, end, hours, tag, what, source (`notes` or `calendar`), counted, bill, billable, file and line.
- **Day**: the entries for a date, its source, its grid code and totals.
- **Week**: Monday to Sunday, ISO numbered. A week spanning two months is summarised in the file of the month it starts in.
- **Month**: the calendar month; its totals and grid cover only its own days.
- **Totals**: hours and billable hours, in all and by tag, everywhere a total appears.

A period is read in whole weeks, with a day either side for the midnight rule, so a spanning week's summary is complete.

## Rules

Each is a function with tests; values in brackets come from config.

- **Tags** match in any case and take the configured spelling [`tags`]; an unconfigured tag is lower case, and a tag written without `@` gains it. A tag spelt more than one way in the Day Notes is flagged.
- **Written hours win** over a row's time range; a difference over a quarter of an hour is flagged. A row with a range and no hours takes the range; a row with neither counts nothing; both are flagged.
- **Not work**: an excluded tag, or a title starting with an excluded prefix, counts as no work [`rules.exclude`].
- **Holds**: calendar blocks starting with a hold prefix count nothing [`rules.zero_hours`].
- **Billable**: each tag has a fraction from 0 to 1, unlisted tags none [`bill`]; a row's `Bill` cell overrides it. Billable hours are counted hours times the fraction. Turning billable hours into days for a day-rate client is the timesheet's job.
- **Weekend tags**: a tag that may not appear on Saturday or Sunday is flagged, not moved [`rules.weekday_only`].
- **Midnight**: a session past midnight counts in full on the day it started (`21:30-03:00`). Rows on the next day that continue a session ending past midnight join it; a block at 00:00 after a gap is its own day's. This holds across week and month boundaries.
- **Calendar time** counts only for a title that starts with a bracketed tag (`[Acme] Workshop` is `@acme`), only for days up to today (planned time is not worked time), and overlapping blocks count once, the overlap reported.
- **A day written in two files** is read from the file whose name covers the date; a copy elsewhere that records different time is flagged.
- **Overlapping Day Notes rows** count their shared time twice and are flagged.
- **Total rows** are not used; one that differs from its rows is flagged.
- **Short and empty days**: a past weekday under the minimum, or with neither notes nor calendar time, is flagged [`rules.min_weekday_hours`].
- **Zones are dated** [`journal.zones`]: from each date on, the journal's wall clock is in that zone. A day starts at the earlier of its own midnight and the previous zone's, so every instant has one journal date.
- **Grid codes**: whole hours, floored: `0` to `9`, then `A` for 10 to `K` for 20, `+` beyond.

## Outputs

- **Generated sections in the Day Notes**, between markers cldr owns: `<!-- cldr:month YYYY-MM -->` under the file's title (the month grid, and hours and billable hours by tag), and `<!-- cldr:week YYYY-Www -->` directly under each week heading (hours by tag and day, and the week's billable hours). Only weeks that have begun are rendered. A week with no heading gets one, under the TWIL heading that opens its block when there is one, in newest-first order. Everything outside the markers is kept byte for byte.
- **Structured data**: `YYYYMMDD-YYYYMMDD-data.json` beside each month's Day Notes, written on every render, with paths relative to the journal; the same document on stdout from `cldr data`. Anything that wants the hours reads this; nothing parses the Markdown back.
- **The year's report**: `YYYY-hours-ytd.md` at the journal root, written whole: hours and billable hours by project and month, and each month's grid by project and day.
- **The Daily Briefing**: the `<!-- gen:NAME -->` blocks of `YYYYMMDD-YYYYMMDD-daily-briefing.md`: today, tomorrow, the week, the month's all-day events, and any configured blocks, each filled from a command's output (eg a list of open projects). Briefing a new day moves the previous Briefing section under `## Earlier`. Calendar lines link to the event (`link`) and its video call (`join`).
- **Calendar CSV**: `cldr export` writes a week or month as `Date,From,To,Title,Modified`, a diagnostic of what the reader sees.

## Commands

```text
cldr render week|month|year [DATE] [--dry-run] [--no-calendar]
cldr briefing [DATE] [--dry-run]
cldr check week|month|year [DATE] [--activity-logs] [--json] [--no-calendar]
cldr data week|month|year [DATE] [--no-calendar]
cldr month new [YYYY-MM] [--dry-run]
cldr export week|month [DATE] [--stdout] [--force] [--dropped]
cldr doctor
cldr paths
```

A DATE may be `YYYY-MM-DD`, or `YYYY-MM` for a month, or `YYYY` for a year. `render week` defaults to last week; `briefing` to today, or tomorrow from the configured cutover hour. `render` is safe at any time. `check --activity-logs` compares each day and tag with the old Activity Logs (read only, kept for the transition).

## Month lifecycle

`cldr month new` works out a plan, prints it, then applies it (`--dry-run` stops after printing):

1. Create the month's files from `[month.templates]`, never overwriting one. A template may use `{{START_DATE}}`, `{{END_DATE}}`, `{{YEAR}}`, `{{MONTH}}`, `{{MONTH_NAME}}` and `{{AUTHOR}}`; any other placeholder is refused. The Day Notes gain a note when their first week is summarised in the previous month.
2. Repoint each `[month.links]` link at the month's file. If a sync service has turned a link into a plain file, it is replaced only when a month file holds exactly the same text.
3. Move every root file named for an earlier month into `YYYY/YYYYMM/`, with `git mv` when git tracks it; a move onto an existing file stops the plan.

Run `cldr render month` on the closing month first; `month new` does not render.

## File names

A date range, then a kebab-case kind: `20261101-20261130-day-notes.md`, `20261101-20261130-data.json`, `20261026-20261101-w44-calendar.csv`, `2026-hours-ytd.md`. The kind is what `[month.templates]` and `[month.links]` name. `src/names.rs` is the one place names are made and read.

## Calendar access

The source is BusyCal's local store, the union of the user's calendar accounts, opened read-only; cldr never writes to it. The reader sits behind a `CalendarSource` trait. The schema is undocumented; what the reader relies on: events are `ZCOMMONDATA` rows (entity 5 an event, 6 a changed occurrence, 8 a calendar); times are seconds since 2001 with a zone name; all-day events sit at noon UTC with an exclusive end; titles and calendar names are keyed archives; repeating events carry a rule the reader expands (frequency, interval, count, until, by-day with ordinals, by-month-day, by-month, set positions, exception and extra dates, changed occurrences); the same invitation on two calendars shares a UID and is reported once; event status 7 is cancelled, attendee status 9 declined; a video call is in `ZCONFERENCECALLEXTRA`, the URL or the location.

On macOS the store is in another app's group container, which needs Full Disk Access for the terminal; an OS update can revoke it, and `cldr doctor` names the setting. EventKit, the macOS calendar layer, is the planned second source.

## Configuration

TOML at `$XDG_CONFIG_HOME/cldr/config.toml` (default `~/.config/cldr/config.toml`); `CLDR_CONFIG` or `--config PATH` overrides it. Unknown keys are refused. [examples/config.toml](../../examples/config.toml) shows every section: `[journal]` (root, author, default zone, templates folder, `[[journal.zones]]`), `[calendar]` (source, include and exclude, the user's addresses), `[tags]`, `[bill]`, `[rules]`, `[month.templates]`, `[month.links]` and `[briefing]` (the cutover hour and `[[briefing.blocks]]`).

## Repository

Edition 2024 with `unsafe_code` forbidden and clippy's `unwrap_used` and `expect_used` denied; `rustfmt.toml` with two-space tabs; `src/` by concern (see `intent/llm/ARCHITECTURE.md`); CI on macOS and Linux; a Homebrew formula that installs the signed and notarised release asset; release tooling in shell. MIT licence.

## Privacy

The repository is public. It holds no names, addresses, client or project names, calendar names, tags or paths from any real journal. Tests build their own synthetic journals and calendar stores; the example config uses placeholders. Everything personal is in the user's config.

## Testing

- Unit tests per rule, including the midnight cases at week and month boundaries.
- Rendering: render, render again, no change; the hand-written text survives intact; weeks not begun are left alone.
- The calendar reader: a synthetic store built in the same schema, covering each recurrence form, exceptions, moved occurrences, all-day spans, merged invitations, declined and cancelled events, links and calls.
- Expected values derive from the input a test builds, never from a count written by hand.
- Acceptance, local only: `cldr check year --activity-logs` over a real journal, every difference from the old logs explained.

## Settled

- D1: the Day Notes are the record; the calendar is the fallback and the cross-check.
- D2: one hand-written file per month, with generated sections; a JSON document per month; no Activity Log.
- D3: zones are dated.
- The current-month files are reached through links at the journal root, repointed each month.
- BusyCal's store is the calendar source; EventKit is the planned second; no direct Google or Microsoft readers.
- Anything specific to one user's setup (eg where their projects are listed) is a configured command, never code.
