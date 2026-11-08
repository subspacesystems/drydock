# m11-gui-env-rename: real-window tests read the old GUI-tests variable

The real-window tests are switched on by `DRYDOCK_GUI_TESTS=1` (the oracle's `test` gate sets it
and runs under Xvfb). The variable used to be `MODELER_GUI_TESTS`. Tests still reading the old
name skip silently in the oracle, so nobody has seen them run.

## Already done on main
`tests/dialog_window.rs` and `tests/dialog_sun.rs` (crate `drydock`) now read
`DRYDOCK_GUI_TESTS`. They have never run in the gate, so some of their real-window checks may
fail now.

## What to do
1. Run, with Xvfb, xdotool and xprop available and `DRYDOCK_GUI_TESTS=1`:
   `xvfb-run -a -s "-screen 0 1280x800x24" cargo test -p drydock --test dialog_window --test dialog_sun`
   Fix the code (not the tests) until they pass: the real-window checks describe the intended
   behaviour of the dialog windows (own title, centred over the main window, closes from its
   title bar, no stray extra window).
2. `tests/title_fresh_doc.rs` (arrives with `m11-title-fresh-doc`, which this task waits for)
   still reads `MODELER_GUI_TESTS`: rename it there (comment and both uses) to
   `DRYDOCK_GUI_TESTS`. It expects a window titled exactly `untitled.drydock - Drydock` and no
   window titled exactly `Drydock`. Run it as above and fix the app if it fails.
3. `grep -rn MODELER_GUI_TESTS app/ docs/specs` must find nothing outside `docs/specs` (old
   specs may keep the old name; leave them).
4. If a test is flaky only because of timing (a wait that is too short under Xvfb), you may lengthen the wait in the helper; never remove an assertion or skip a test.
5. Run `harness/bin/run-tests -p drydock` with `DRYDOCK_GUI_TESTS=1` under Xvfb at the end.

## Where to look
- `show wait_until`, `show windows` (xdotool search helpers) in `tests/title_fresh_doc.rs`
- `show dialog_window` and the real-window tests in `tests/dialog_window.rs`, `tests/dialog_sun.rs`
- `dialog_window` helper module in `app/crates/drydock/src/`; `harness/bin/api-map drydock`
- Startup title: search `untitled.drydock` in `app/crates/drydock/src/`
- Gate: `harness/oracle/gates.sh` (the `test` gate)
