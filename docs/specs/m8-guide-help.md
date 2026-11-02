# m8-guide-help: Help > User Guide (F1)

Help gets a **User Guide** entry (shortcut **F1**) that opens the bundled guide's start page in the default browser. The skeleton is on main (`guide_help.rs`, `Action::UserGuide`, `Effect::UserGuide`); the tests are `tests/guide_help.rs` and `tests/guide_help_ui.rs`. Use the name Drydock in all text.

## What to build
1. `guide_help.rs` (fill in the `unimplemented!` bodies):
   - `guide_candidates(exe_dir)`: with `Some(dir)`: `dir/guide/index.html` first, then the dev path. With `None`: only the dev path. The dev path is `<CARGO_MANIFEST_DIR>/../../target/guide/index.html` (build it with `env!("CARGO_MANIFEST_DIR")`), the folder `drydock-guide docs/guide app/target/guide` writes to. Use `GUIDE_INDEX` for the file name.
   - `find_guide`: first candidate where `exists` is true.
   - `browser_command(os, page)`: Linux `xdg-open PAGE`; Mac `open PAGE`; Windows `cmd` with args `/C`, `start`, `""` (an empty argument), PAGE. Program name is a `String`, arguments `OsString`s.
   - `SystemBrowser::open`: build the command with `Os::current()`, spawn it with `std::process::Command` (stdio null, don't wait), map a spawn error to `Err(error.to_string())`. On Windows set `CREATE_NO_WINDOW` through `std::os::windows::process::CommandExt` under `#[cfg(windows)]` so no console flashes.
   - `open_guide`: if not found, return `"The Drydock User Guide was not found."` followed by a space and `Looked in:` and the candidate paths joined with `, `. If found, call the launcher: on success return `"Opened the Drydock User Guide in your browser."`, on error `"Could not open the Drydock User Guide: {reason}"`.
2. `input::key`: plain `"F1"` gives `Action::UserGuide` (Shift/Ctrl/Alt+F1 give nothing).
3. `viewstate::ViewState::apply`: `Action::UserGuide` returns `Effect::UserGuide` (replace the `unimplemented!`).
4. `chrome::menu_bar`: Help menu, **first** item `menu_item(ui, None, "User Guide", "F1", false)` choosing `Action::UserGuide` and closing the menu; About stays below it. No ellipsis (it opens a browser, not a window).
5. `app.rs`, `Effect::UserGuide`: call `open_guide(&guide_candidates(exe_dir), |p| p.is_file(), &SystemBrowser)` with `exe_dir` from `std::env::current_exe()` (parent folder), and show the returned text in the status bar by storing it in `export_note` (the existing status line; no new field needed). Keep this a small private method.

## Out of scope
- The installer script and `packaging/release.sh` must copy the built guide into `<install dir>/guide/` (the layout the first candidate expects); other M8 tasks do that. Do not change them here.
- Do not write guide pages or change `drydock-guide`.

## Where to look
`show` these: `Action`, `Effect`, `ViewState::apply`, `input::key`, `chrome::menu_bar` (Help menu near the end), `chrome::menu_item`, `Os::current` (`look.rs`), `export_note` uses in `app.rs`; the skeleton `guide_help.rs`. Test helpers: the `harness`/`open_help` functions in `tests/guide_help_ui.rs`; `menus` in `tests/menus_fit.rs` for the same pattern.
