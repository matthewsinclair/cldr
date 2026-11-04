---
verblock: "2026-11-04T07:59:29Z:v0.2: matts - Cldr at v0.2.1, and what is open"
---

# Work In Progress

DOING and TODO only; finished work lives in its commit, its closed issue or its satisfied criterion. Lead each focus item with its thread id in bold, the thread's digits in place of `####`: the SessionStart hook shows the first such line as the session's `WIP:` line.

## Current Focus

**Cldr stands at v0.2.1.** Nothing is in progress, and no steel thread exists yet: the next piece of work starts one with `intent st new`. A release is the one command, `bin/devbin release vX.Y.Z`, as `intent/llm/RULES.md` describes.

## Active Steel Threads

_(none)_

## Upcoming Work

- The open work in TN0002 (`docs/notes/tn0002-cldr-build-plan.md`, under `## Open`). Its CLI tests meet devbin's `check autotests`: `tests/` already holds `tests/packaging.rs`, and a crate with more than one auto-discovered test target fails the check, so the CLI tests join that target or are declared with `[[test]]`.
- `fact()`'s lost status in `bin/.devbin/cmd/macos`: a `local x=$(fact ...)` keeps no status, so a missing fact reads as empty. It waits for the notary-profile change.
- Intent's shell critic has not yet run over `bin/.devbin/cmd/post-cut`, `macos` and `lock-sync`. Its first findings on them go to pplr-vc before those files change.

## Notes

- `AGENTS.md` is generated, and its `bats -r tests/` and POSIX shell lines do not fit Cldr, as `intent/llm/RULES.md` says. The template fix is Intent's.
