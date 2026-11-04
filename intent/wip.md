---
verblock: "2026-11-04T09:15:34Z:v0.5: matts - Cldr at v0.2.2: what is open"
---

# Work In Progress

DOING and TODO only; finished work lives in its commit, its closed issue or its satisfied criterion. Lead each focus item with its thread id in bold, the thread's digits in place of `####`: the SessionStart hook shows the first such line as the session's `WIP:` line.

## Current Focus

**Cldr stands at v0.2.2.** Nothing is in progress, and no steel thread exists yet: the next piece of work starts one with `intent st new`.

## Active Steel Threads

_(none)_

## Upcoming Work

- The open work in TN0002 (`docs/notes/tn0002-cldr-build-plan.md`, under `## Open`).
- Decide whether `cldr export` should write its CSV through a temporary file, as `render` and `briefing` do; it writes directly (`src/cli.rs`).
- `fact()`'s lost status in `bin/.devbin/cmd/macos`: a `local x=$(fact ...)` keeps no status, so a missing fact reads as empty. It waits for the notary-profile change, which comes after the patch cuts.
- Intent's shell critic has not yet run over `bin/.devbin/cmd/post-cut`, `macos` and `lock-sync`.
- The tap's CI excuses devbin's long manifest line, with `--except-cops Layout/LineLength` and `--skip-style` (homebrew-cldr `8f7df94` and `d938ad9`); both go once devbin's `finish_ruby` wraps the line it writes.
- Nothing runs the daily store backup that `intent/.config/config.json` declares: run `intent backup` by hand when `intent doctor` says backup-stale.
