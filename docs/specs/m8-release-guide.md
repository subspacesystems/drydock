# m8-release-guide: the release script builds the user guide

`packaging/release.sh` ships a guide only when `--guide DIR` is given. Without it the
installer has no guide and F1 reports that the User Guide was not found. Change the script
so a plain release builds the guide itself.

## Behavior
- `--guide DIR` given: unchanged. The folder must exist (checked before anything runs), is
  copied to `<stage>/guide`, and no guide is built.
- `--guide` absent: after the Windows build and the `rm -rf`/`mkdir` of the stage, run
  `"$cargo" run --release --locked -p drydock-guide -- <app_dir>/../docs/guide <stage>/guide`
  from `app_dir`. No `--target`: the generator runs on the host. The same `CARGO_TARGET_DIR`
  is already exported. If it fails, the release fails (the script is `set -e`; nothing
  after it, makensis included, runs and no SHA256SUMS is written).
- `--dry-run`: print the guide command with `print_cmd` (a `+ ` line), after the Windows
  build line and before the makensis line, only when `--guide` is absent. Nothing runs.
- Use an absolute path for the docs folder (resolve `app_dir/../docs/guide` with `cd`/`pwd`
  or `realpath`), ending in `docs/guide`.
- The installer script already packs `${STAGE}\guide`; do not touch it.

## Tests
`app/crates/drydock/tests/release_script.rs` (already updated, do not edit): the stub cargo
now handles `-p drydock-guide` (writes `index.html` into its last argument). New or changed
tests cover: guide built into the stage when `--guide` is absent, no build when it is given,
failure of the build fails the release, and the dry-run plan.

## Where to look
- `app/packaging/release.sh`: the `guide` variable, `print_cmd`, the dry-run block and the
  `rm -rf "$stage"` / `cp -R "$guide"` part.
- `Rig`, `CARGO_STUB`, `MAKENSIS_STUB` in `release_script.rs`.
