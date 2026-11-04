# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - unreleased

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
