---
verblock: "2026-11-04T19:53:49Z:v0.6: matts - ST0001 in review, ST0002 next"
---

# Work In Progress

DOING and TODO only; finished work lives in its commit, its closed issue or its satisfied criterion. Lead each focus item with its thread id in bold, the thread's digits in place of `####`: the SessionStart hook shows the first such line as the session's `WIP:` line.

## Current Focus

**ST0001** `cldr export` writes through a temporary file: landed, reviewed by cldr-vc; its ACs and three small test and cleanup findings remain (the thread's Context lists them).

**ST0002** CLI tests: next; hv ruled that the synthetic calendar store builder is shared with them (the thread's Context).

## Active Steel Threads

- ST0001: cldr export writes through a temporary file (dc).
- ST0002: CLI tests: the binary run over a generated journal (dc).

## Upcoming Work

- The rest of TN0002's Open section (`docs/notes/tn0002-cldr-build-plan.md`): EventKit, `month new` checking first, retiring `legacy.rs`.
- `bin/.devbin/cmd/post-cut`, `macos` and `lock-sync`: devbin builds their replacement (hv's ruling); cldr-dc's critic-shell findings went to devbin-vc. Edit none of them meanwhile.
- The tap's CI excuses devbin's long manifest line, with `--except-cops Layout/LineLength` and `--skip-style` (homebrew-cldr `8f7df94` and `d938ad9`); both go once devbin's `finish_ruby` wraps the line it writes.
- Nothing runs the daily store backup that `intent/.config/config.json` declares: run `intent backup` by hand when `intent doctor` says backup-stale.
