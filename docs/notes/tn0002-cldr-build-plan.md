# TN0002: Cldr build plan

Status: in progress. Design: [TN0001](tn0001-cldr-design-note.md).

Each step ends with something usable on a real journal, so the old tooling can be retired one piece at a time. Sizes are relative (S, M, L).

## Step 0: the project (S)

- Cargo project after Ccusage: edition 2024, lints, `rustfmt.toml`, MIT licence, README, `.gitignore`.
- `cldr paths` and `cldr doctor` (config found, journal root exists, calendar store readable, with the macOS setting named when it is not).
- Config loading from XDG with `CLDR_CONFIG` and `--config` overrides; `examples/config.toml`.
- CI: build, test, clippy and fmt on macOS and Linux.

Done when: `cldr doctor` reports a real setup correctly, and CI is green.

## Step 1: the calendar reader and `cldr export` (M)

- `CalendarSource` trait; `BusyCal` reader over the store, read-only.
- Keyed-archive decoding for titles, calendar names and the rule's integer lists.
- Times: seconds since 2001 plus zone; all-day spans; rendering in the configured zone.
- Recurrence: daily, weekly, monthly, yearly; interval, count, until, by-day (with ordinals), by-month-day, by-month, set positions; exception dates; changed occurrences replacing their originals; extra dates.
- Filters: hidden calendars and task lists skipped; declined and cancelled dropped by default; the same UID on two calendars merged; include and exclude by account or calendar name.
- `cldr export week|month [DATE]`: the calendar as CSV in the old format, written to the journal's month folder (the Sunday's month for a week), never overwriting without `--force`. A diagnostic now that the old wraps are retired, and the first proof of the reader.
- Tests over a synthetic store the tests build in the same schema.

Done when: a week exported by `cldr` matches the same week read by the old reader, event for event.

## Step 2: the Day Notes parser and the model (M)

- Parse each day's heading and `Time Log` table; keep the source line of every row for error messages.
- Entries, days, weeks, months; tag normalisation; the rules in TN0001, each with tests.
- `cldr data week|month|year --json`.

Done when: `cldr data month` for a real month gives totals that can be checked by hand against its Day Notes.

## Step 3: the month lifecycle (S)

Brought forward: the old wraps are not run again, so months wait for this.

- `cldr month new`: templates, spanning-week note, the archive move (`git mv` in a work tree), the briefing symlink. The final render and check join it after step 5.
- Turns over every month still waiting, oldest first.

Done when: a month turns over with one command and nothing else to do by hand.

## Step 4: the year check (M)

- `cldr check year`: every week and month, with flags and the consistency list.
- A comparison against the old Activity Logs (read only, for the transition): per day and per tag, with each difference listed.
- Work through the differences with the user: fix `cldr`, or record the old log's error.

Done when: every difference across the year is explained.

## Step 5: rendering into the Day Notes (M)

- Generated sections between markers: the month grid and totals at the top, a summary under each week heading.
- The month's JSON beside its Day Notes.
- `cldr render week|month`; `check` as the dry run. Round-trip tests: render twice, no change; text outside markers unchanged.

Done when: a real month renders cleanly. From here `month new` also does its final render and check before archiving.

## Step 6: the briefing, and retiring the old tooling (M)

- `cldr briefing`: the generated blocks of the Daily Briefing, as the old briefing script writes them.
- Remove the old scripts, prompts, Python helpers and `_processing/` files from the journal.
- Release: tags, binaries, Homebrew formula, as Ccusage does.

Done when: the journal's `bin/` is gone.
