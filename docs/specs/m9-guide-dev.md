# m9-guide-dev: Help > User Guide from a plain cargo build

Tyrel built Drydock with `cargo build` from git and Help > User Guide (F1) found nothing: only `packaging/release.sh` ran `drydock-guide`, and `guide_candidates` looked for `app/target/guide/index.html`, which a cargo build never creates (a custom `CARGO_TARGET_DIR` would miss it too). Fix: the drydock crate's build script builds the guide itself.

## What to build
1. `crates/drydock/build.rs` (already has the git-hash and Windows-resource steps; keep them):
   - Source folder: `<CARGO_MANIFEST_DIR>/../../../docs/guide` (the repo's `docs/guide`). Output: `$OUT_DIR/guide`.
   - If the source folder has no `SUMMARY.md` (crate built outside the repo), do nothing for the guide: no env var, no failure, no warning noise.
   - Otherwise delete `$OUT_DIR/guide` if it exists (stale pages from removed sources), call `drydock_guide::build(&src, &out)`, and on `Err` panic with the error's text (a broken guide should fail the build: the guide has its own tests, so this is rare). Print `cargo:rustc-env=DRYDOCK_DEV_GUIDE=<out path>`.
   - Print `cargo:rerun-if-changed=` for the source folder (cargo scans a folder recursively) so editing a page rebuilds it. Do not print rerun lines for `winmeta.rs` changes beyond what exists.
   - `drydock-guide` is already a build-dependency in `Cargo.toml` (on main). It depends only on `pulldown-cmark`, so there is no cycle.
2. `guide_help.rs`, `guide_candidates(exe_dir)`: installed layout `exe_dir/guide/index.html` first (when `exe_dir` is `Some`), then the dev guide `$DRYDOCK_DEV_GUIDE/index.html`, read at compile time with `option_env!("DRYDOCK_DEV_GUIDE")`. When the variable is absent the dev candidate is simply left out (so `guide_candidates(None)` can be empty; `open_guide` already reports "not found" with the list). Update the doc comment. Do not mention `app/target/guide` any more.
3. `packaging/release.sh` keeps staging its own guide next to the executable; don't change it. The release and cross builds must still work: the build script runs on the host, so it needs nothing target-specific. Check `cargo clippy --workspace --all-targets -- -D warnings` and the Windows cross-check command used by `winclippy` (find it in `harness/bin` or the packaging scripts) still pass.

## Tests
`tests/guide_help.rs` (updated) and `tests/guide_dev.rs` (new) read `env!("DRYDOCK_DEV_GUIDE")`, so they only compile when the build script exports it. Do not edit them.

## Out of scope
Guide content, `drydock-guide` itself, the installer.

## Where to look
`show` these: `guide_candidates`, `find_guide`, `open_guide` (`guide_help.rs`); `drydock_guide::build` and `BuildReport` (`drydock-guide/src/lib.rs`); `main` in `crates/drydock/build.rs`. The workspace has `drydock-guide` as a path dependency in `app/Cargo.toml`.
