# TN0002: Cldr build record

Status: steps 0 to 6 built; v0.2.0 released. Open work is at the end. Design: [TN0001](tn0001-cldr-design-note.md).

Each step ended with something usable on a real journal, so the old tooling could be retired one piece at a time.

## What was built

| Step | What                                                                                                                                                                                                                                                                                       | Commits                       |
|------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------|
| 0    | The project after Ccusage: lints, config from XDG with `CLDR_CONFIG` and `--config`, `cldr paths`, `cldr doctor`                                                                                                                                                                            | `fd254c1`                     |
| 1    | The BusyCal reader behind `CalendarSource` (keyed archives, times and zones, recurrence, filters, merged invitations) and `cldr export`; matched the old reader week for week across a year                                                                                               | `fd254c1`                     |
| 2    | Dated zones; the lenient Day Notes parser; the model and its rules; `cldr data`                                                                                                                                                                                                            | `db8b2e3` `da3095c`           |
| 3    | `cldr month new`: plan, templates, spanning-week note, links, archive by `git mv`                                                                                                                                                                                                          | `271be1b`                     |
|      | File names as a date range and a kebab-case kind (`names.rs`); billable hours (`[bill]` and the optional `Bill` column); CI, the formula template and shell release tooling                                                                                                                | `ee15c1f` `991a868` `df5d4f2` |
| 4    | `cldr check`, and the comparison with the old Activity Logs (`legacy.rs`, judged by the model's own `Rulebook`); the `notes_overlap` flag. Over a year of real data most days agreed, and every difference was explained (the old wrap retimed from the calendar, or a deliberate retag) | `574e8d7` `0d4054d`           |
| 5    | `cldr render week` and `render month`: the month and week sections and the month's JSON; render twice, no change                                                                                                                                                                          | `363fd2b` `1766f11`           |
| 6    | `cldr briefing` (a port of the old briefing script, with configured command blocks), `link` and `join` on events, `cldr render year`; the old journal tooling removed                                                                                                                      | `2115542`                     |

Releases: v0.1.0 (steps 0 to 3) and v0.2.0 (steps 4 to 6), tagged on GitHub with the formula in the `homebrew-cldr` tap.

## Lessons kept

- Test on a copy of a real month before writing into the real journal: the first real render found two placement bugs (a new week heading above the TWIL heading; sections for weeks not yet begun) that synthetic journals had not.
- Compare day copies by what they record, not by line; copies of a day sit on different lines in different files.
- A dry run of a ported command over a real day, diffed against the old command's output, is the acceptance test for a port.

## Open

- **EventKit as a second source** (was step 3b): a reader behind `CalendarSource`, chosen by `calendar.source = "eventkit"`, its `unsafe` bindings in one module; `cldr export` from both sources across a year, every difference explained, then EventKit as the default.
- **`month new` renders and checks first**: render the closing month and stop if `check` reports what the user has not seen, before archiving it.
- **CLI tests**: tests under `tests/` that run the binary over a generated journal, beside the unit tests.
- **Retire `legacy.rs` and `check --activity-logs`** once no journal still compares against old Activity Logs.
