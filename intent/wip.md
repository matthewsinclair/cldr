---
verblock: "2026-11-06T13:41:51Z:v0.7: matts - ISSUE:0001 held for devbin's #0326 release, then the one cut"
---

# Work In Progress

DOING and TODO only; finished work lives in its commit, its closed issue or its satisfied criterion. Lead each focus item with its thread id in bold, the thread's digits in place of `####`: the SessionStart hook shows the first such line as the session's `WIP:` line.

## Current Focus

**ISSUE:0001** Cldr adopts devbin's dev/opt/brew core: held until a devbin release carrying devbin#0326 is vendored into Cldr (devbin-vc's todo 201 says when). Then cldr-dc adopts three things, each listed with its lines in the issue's record (`intent issues show 0001`): the `install:` block in `bin/.devbin/config.yaml`, with `formula.bin` dropped; the `--version` reader, which devbin-cc sends word for word; and the formula on `@@INSTALL@@`, with caveats naming `bin/devbin use`. cldr gains no `install`, `use` or `version` verb: the publish and the switch are `bin/devbin install` and `bin/devbin use`, run in the checkout. cldr-vc reads the diff and dc's one measured run before it lands.

## Active Steel Threads

None.

## Upcoming Work

- The one release, once ISSUE:0001 is on main, with every outstanding item in it (hv): hv's full suite (`bin/devbin check all` and `test all`) first. The version is hv's call: `CHANGELOG.md` has 0.2.3 open, and a new `--version` line and formula may argue for 0.3.0.
- The rest of TN0002's Open section (`docs/notes/tn0002-cldr-build-plan.md`): EventKit, `month new` checking first, retiring `legacy.rs`.
- `bin/.devbin/cmd/post-cut`, `macos` and `lock-sync`: devbin builds their replacement (hv's ruling); cldr-dc's critic-shell findings went to devbin-vc. Edit none of them meanwhile.
- The tap's CI excuses devbin's long manifest line, with `--except-cops Layout/LineLength` and `--skip-style` (homebrew-cldr `8f7df94` and `d938ad9`); both go once devbin's `finish_ruby` wraps the line it writes.
- Cldr's clone has no pre-commit hook, so no gate runs on a commit; devbin-vc put it to hv on 2026-11-06. `intent claude upgrade --apply` installs it, at hv's word.
- Nothing runs the daily store backup that `intent/.config/config.json` declares: run `intent backup` by hand when `intent doctor` says backup-stale.
