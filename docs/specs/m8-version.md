# m8-version: one version everywhere

Tyrel's try of M8: Help > About said "0.0.1 (unknown)" and the program's file
properties said 0.0.1, while Add/Remove Programs showed the installer's 0.8.x. All four must
show the same release version, and About must show the commit.

## Behaviour

- **Version variable.** `DRYDOCK_VERSION` in the build environment is the release version.
  When it is unset or blank, the Cargo package version is used. `build.rs` decides with
  `winmeta::build_version(requested, cargo_version)` (skeleton on main): the requested value is
  trimmed and must pass `file_version`, else the function returns `Err` whose text names the
  value, and `build.rs` panics with it (a typo must stop the build, not ship 0.0.0.0).
- **`build.rs`** emits `cargo:rustc-env=DRYDOCK_VERSION=<version>` and
  `cargo:rerun-if-env-changed=DRYDOCK_VERSION`, and feeds the Windows version resource
  (`file_version`, `version_strings`) from that same value, not from `CARGO_PKG_VERSION`.
  Remove the `#[allow(dead_code)]` on `build_version`.
- **The program** shows it: `chrome::VERSION` becomes `env!("DRYDOCK_VERSION")`; `about_text`
  and `drydock::banner()` (in `lib.rs`) use it. Nothing formats `CARGO_PKG_VERSION` directly.
- **Workspace version** is 0.8.0 (done on main, with `Cargo.lock`): M8 is 0.8.x, so a plain
  build is not 0.0.1.
- **`app/packaging/release.sh`** exports `DRYDOCK_VERSION=$version` for the cargo build (the same
  value it gives makensis and the file name).
  It also exports `DRYDOCK_GIT_HASH`, unless already set and non-empty, from
  `git -c safe.directory="$repo" -C "$repo" rev-parse --short HEAD`, where `$repo` is the
  parent of `$app_dir`. (The try build ran in a container with the checkout owned by another
  user, so plain `git` refused and About said "unknown".) If git fails or prints nothing,
  leave it unset and carry on: `build.rs` then falls back as today.
  `--dry-run` prints the lines `DRYDOCK_VERSION=<v>` and, when known, `DRYDOCK_GIT_HASH=<h>`
  (exactly these, one per line, on stdout) before the cargo build line, and still runs nothing.
- The two older assertions on `CARGO_PKG_VERSION` in `tests/chrome.rs` (about text) and
  `tests/drydock_text.rs` keep passing only while the two values agree; change them to
  `drydock::chrome::VERSION`.

## Tests

`app/crates/drydock/tests/version_everywhere.rs` (spec commit): `build_version` cases; About and
banner show `chrome::VERSION`, which equals `option_env!("DRYDOCK_VERSION")` (set by build.rs); source checks on
`build.rs`; `release.sh` run from a temporary copy of the checkout with stub `cargo`/`makensis`
and a temporary git repository (hash, kept preset hash, no repository, dry-run lines, default
version from `Cargo.toml`). Existing tests in `release_script.rs` must keep passing.

## Where to look

- `show winmeta::build_version`, `show winmeta::file_version`, `show winmeta::version_strings`
- `app/crates/drydock/build.rs` (hash logic and the Windows resource block)
- `show chrome::about_text`, `chrome::VERSION`, `drydock::banner` (lib.rs)
- `app/packaging/release.sh` (version parsing, `print_cmd`, the dry-run block and the real
  build block); the stub-cargo pattern in `tests/release_script.rs` (`Rig`, `CARGO_STUB`)
