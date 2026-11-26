# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] - unreleased

### Changed

- cldr follows devbin's dev/opt/brew (devbin#0326, cldr#0001). `cldr --version` prints the version line, `cldr <version> (<tree>, <sha12>[+dirty])`: the line `bin/devbin build` stamps, or in an opt or brew tree the one its manifest records. A plain `cargo build` prints `(unknown, unknown)`. `bin/devbin install` publishes an opt tree and `bin/devbin use dev|opt|brew` switches `~/.local/bin/cldr`, both run in a checkout; cldr has no verb of its own for either.
- The formula installs through devbin's `@@INSTALL@@`, keeps `share/cldr/config.toml`, and its caveats name `bin/devbin use brew`.
- `docs/releasing.md` is the maintainer's page: the dev, opt and brew trees, building and testing, signing, and cutting a release. `bin/devbin t10-probe` is devbin's T10 proof for cldr: `all` (the default), `dev-opt` or `brew` runs those legs, and it ends with one verdict line.
- `cldr export` writes its CSV through a temporary file and a rename, as `render` and `briefing` do, so an interrupted export never leaves half a CSV behind. When it cannot create the folder or write the file, the error names it.
- A failed write or rename in `render`, `briefing` and `export` removes its temporary file, and the error says so when it cannot. The temporary file is synced to disk before it replaces the target, so a write the disk refuses late fails instead of leaving a short file in the target's place.

## [0.2.2] - 2026-11-04

### Changed

- Documentation only: the README and the example config now match cldr as built. Only `render` and `briefing` write through a temporary file, and the example config shows `calendar.store` and explains `include`, `exclude` and `me`.

## [0.2.1] - 2026-11-04

### Changed

- Homebrew installs the signed and notarised macOS binary from the release asset instead of building cldr from source, and checks the binary's signature at install and in `brew test`.

## [0.2.0] - 2026-11-01

### Added

- `cldr check`, and the comparison with the old Activity Logs.
- `cldr render week`, `render month` and `render year`.
- `cldr briefing`, and `link` and `join` on events.

## [0.1.0] - 2026-11-01

### Added

- The BusyCal reader and `cldr export`; `cldr data`, `cldr month new`, `cldr paths` and `cldr doctor`.
