---
verblock: "2026-11-04T05:24:12Z:v0.1: matts - Cldr's rules for changes, and how to work and release, moved from AGENTS.md"
---

# RULES.md

Cldr's own rules for changing the code, and how to work on it and release it. The cross-language principles, Highlander, PFIC, Thin Coordinator and No Silent Errors, are stated in `AGENTS.md` under "Rules of the Road"; Cldr's homes for them, and the invariants these rules protect, are in `intent/llm/ARCHITECTURE.md`.

## Rules for changes

- No personal data, ever: this repository is public. No real names, clients, tags, calendars or paths, in code, tests, docs or commit messages. Tests build synthetic journals and stores; use `@acme` and `example.com`.
- No `unwrap`, `expect` or `unsafe` (the lints deny them); errors are the crate's `Result` with a message that names the file or setting.
- No magic numbers in tests: an expected value derives from the input the test builds.
- No Python anywhere in the repository; tooling is shell or Rust.
- Anything specific to one user's setup is a configured command or a config value, never code.
- A new rule or flag gets a test and a line in TN0001; a new command gets its line in `cli.rs`'s help, the README and TN0001.

## Working on it

```sh
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
cldr --config /path/to/a/copy/config.toml render month 2026-10 --dry-run
```

Try anything that writes on a copy of a real journal first (a config whose `journal.root` points at the copy), and diff the copy against the original: nothing outside the markers may change. Reading the calendar needs Full Disk Access for the terminal.

`AGENTS.md`'s generated `bats -r tests/` and its line on POSIX shell do not apply here: Cldr has no bats suite, its tests run through `bin/devbin test all`, and its scripts are zsh by design.

## Releasing

Open `## [X.Y.Z] - unreleased` in `CHANGELOG.md`, then run the one command, `bin/devbin release vX.Y.Z` (`--dry-run` first), on the maintainer's Mac, with the Developer ID identity and the `geodica.apple.*` notary credentials in the login keychain. It stamps the version into `Cargo.toml`, brings `Cargo.lock` in step (`bin/devbin lock-sync`) and dates the CHANGELOG heading; runs the gates (`check all`, `test all`, `doctor`) and CI on the `candidate/vX.Y.Z` branch; then tags, pushes and makes the GitHub release. Its `after:` step, `bin/devbin post-cut`, builds the tag (`bin/devbin build`), signs and notarises `cldr` (`bin/devbin macos prepare`), attaches the asset with `SHA256SUMS` and publishes the formula to the tap, stopping at the first failure; once the cause is fixed, `bin/devbin post-cut vX.Y.Z` resumes from the tag. pplr releases the same way. Upgrading an installed copy needs `brew update` first, so Homebrew sees the tap's new formula.
