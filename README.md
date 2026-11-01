# Cldr

A re-runnable renderer for a plain-text time journal. Cldr reads the Day Notes you write and your calendar, and derives everything else from them: week and month summaries in the Day Notes, structured JSON, and the Daily Briefing's generated blocks. Rust. No language model anywhere in the pipeline.

Status: early. `cldr export`, `cldr doctor` and `cldr paths` work; the rest is planned in [TN0002](docs/notes/tn0002-cldr-build-plan.md). The design is [TN0001](docs/notes/tn0001-cldr-design-note.md).

## Build and try

```sh
cargo build --release --locked
cp examples/config.toml ~/.config/cldr/config.toml   # then edit
target/release/cldr doctor
target/release/cldr export week --stdout
```

## Configuration

`$XDG_CONFIG_HOME/cldr/config.toml` (default `~/.config/cldr/config.toml`); `CLDR_CONFIG` or `--config PATH` overrides it. See [examples/config.toml](examples/config.toml). Everything personal lives in the config, never in this repository.

## Calendar access

The calendar is read from BusyCal's local store, which holds every account BusyCal syncs. The store is only ever read. On macOS, the terminal running `cldr` needs Full Disk Access to read it; `cldr doctor` says so when it is missing.

## Licence

MIT.
