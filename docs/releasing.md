# Releasing cldr

This page is for the maintainer of a cldr checkout: its trees, building and testing, signing, and cutting a release. A user installs cldr with Homebrew alone, as the README says.

cldr follows devbin's dev, opt and brew contract (T1 to T10 in devbin's `docs/dev-opt-brew.md`) through devbin's own implementation (devbin#0326, cldr#0001). `bin/devbin install` is the publish and `bin/devbin use` is the switch. Run each in this checkout. cldr has no verb of its own for either. Its `install:` block in `bin/.devbin/config.yaml` declares the opt prefix, the link and the example config.

## The three trees

| Tree | Where                                                                | Written by                         |
| ---- | -------------------------------------------------------------------- | ---------------------------------- |
| dev  | the checkout, with its build in `target/release/`                    | `bin/devbin build`                 |
| opt  | `install.prefix` in `bin/.devbin/config.yaml`, eg `~/Devel/opt/cldr` | `bin/devbin install`, run from dev |
| brew | `$(brew --prefix)/opt/cldr/libexec`                                  | the formula                        |

A tree holds `bin/cldr` and `config.toml`, beside its manifest, `.cldr-install`, which names each file with its sha256. `bin/devbin doctor`, run in the checkout, checks each tree against its manifest and names a file that has drifted. `cldr --version` names the tree it runs from and the commit it was built from, eg `cldr 0.2.3 (dev, 1a2b3c4d5e6f)`, with `+dirty` on a dev build made from uncommitted changes. `bin/devbin build` stamps the line. A plain `cargo build` prints `(unknown, unknown)`, and `bin/devbin doctor` fails it.

- **dev**: `bin/devbin build` runs `cargo build --release --locked` into `target/release/`, which `bin/devbin use dev` links.
- **opt**: `bin/devbin install [--prefix <dir>] [--force]` publishes the dev build as a working copy. This keeps what works apart from what is mid-development. It builds first, proves the binary by its `--version`, and refuses a build made from uncommitted changes. It replaces an opt tree only with `--force`, and only a tree it wrote.
- **brew**: the formula, through devbin's `@@INSTALL@@`, lays the release's signed binary out in the keg, writes the manifest, links `cldr` into Homebrew's `bin` and checks the signature. It also links `share/cldr/config.toml`, the path the README gives.

## Switching trees

`bin/devbin use dev|opt|brew`, run in the checkout, moves one link, `~/.local/bin/cldr`. `bin/devbin use brew` removes the link, so Homebrew's `cldr` answers. `bin/devbin use` with no argument says which tree the link serves. The switch rests at brew. Move to dev or opt only while you work on cldr, and go back with `bin/devbin use brew` when the work is released. `~/.local/bin` must come before Homebrew's `bin` on `PATH`, and `bin/devbin doctor` names a link that something shadows. `bin/devbin help use` gives the rest.

To upgrade each tree:

- **dev**: `git pull`, then `bin/devbin build`.
- **opt**: from dev, `bin/devbin install --force`.
- **brew**: `brew upgrade cldr`.

`bin/devbin t10-probe` is devbin's T10 proof for cldr. It publishes the opt tree, then for dev, opt and brew in turn runs `bin/devbin use`, checks that `cldr --version` names that tree, and runs `bin/devbin doctor`. `bin/devbin t10-probe dev-opt` runs the dev and opt legs alone, and `bin/devbin t10-probe brew` runs the brew leg alone. The brew leg passes only after Homebrew installs a release that carries this cldr. The probe moves `~/.local/bin/cldr` and ends at brew.

## Building and testing

```bash
bin/devbin build            # cargo build --release --locked, with the version line stamped
bin/devbin check all        # every read-only gate; bin/devbin help check names them
bin/devbin test all         # cargo test
bin/devbin doctor           # devbin's own install, and cldr's switch, trees and version line
```

Building needs the Rust toolchain (`Cargo.toml` names the edition) and devbin.

## Signing

The macOS binary a release ships is signed with the Developer ID, with the hardened runtime and a secure timestamp, and notarised. `bin/devbin macos` does it, and its header lists its steps. It runs on the maintainer's Mac only. It refuses on CI, off macOS, and without the signing identity in the keychain. `packaging/macos/signing` holds the identity, the binary's identifier and designated requirement, and the keychain prefix of the notary credentials. The formula template repeats the designated requirement, and a cargo test fails when the two differ.

```bash
bin/devbin build
bin/devbin macos prepare    # stage, sign, verify, notarise, then pack the asset alone into dist/release
```

`bin/devbin macos prepare --adhoc` runs the same steps with an ad hoc signature and a dry run of the notarisation, and packs into `dist/adhoc`. Use it to prove the steps on a scratch build. A release is never ad hoc.

## Cutting a release

A release is one command, on the maintainer's Mac, with the Developer ID identity and the notary credentials in the login keychain.

1. Open the release's section in `CHANGELOG.md` as `## [X.Y.Z] - unreleased`. The cut dates it, and its text is the release's notes.
2. Run `bin/devbin release vX.Y.Z` (`--dry-run` first shows each step). It asks once, then sets the version (`Cargo.lock` follows through `bin/devbin lock-sync`), commits, and runs the gates here and CI on `candidate/vX.Y.Z`. When every gate is green, it tags, pushes and makes the release on GitHub. Its `after:` step, `bin/devbin post-cut`, then builds the release at the tag, signs and notarises `cldr` (`bin/devbin macos prepare`), and attaches the asset with `SHA256SUMS`. It then publishes the formula to the tap, `matthewsinclair/homebrew-cldr`, under the same yes, and pulls this machine's clone of the tap so that `brew upgrade` sees the release.
3. Run `brew upgrade cldr` (`brew install matthewsinclair/cldr/cldr` the first time), then `bin/devbin use brew` in the checkout, then `cldr --version`, which must read brew at the new version.

A step that fails stops every step after it and names the command that resumes: `bin/devbin release vX.Y.Z` again before the tag, `bin/devbin post-cut vX.Y.Z` after it. `bin/devbin help release` and `bin/devbin post-cut --help` describe both. The formula installs the macOS asset for Apple silicon alone.
