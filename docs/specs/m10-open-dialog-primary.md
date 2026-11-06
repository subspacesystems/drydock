# m10-open-dialog-primary: the Open / Import dialog's confirming button in the action color

The Save / Export file dialog already draws its confirming button with `look::primary_button`
(violet fill, `on_action` label; `m10-brand-dialog-buttons`). The Open / Import file dialog
(`OpenDialog::show` in `app.rs`, Modal id "open-dialog", titles "Open model",
"Import into model", "Import component") still uses `ui.button`. Make it match.

## What to build

The button row moves into the library so tests can drive it. On main already: the new module
`open_buttons.rs` with `OpenMode` (moved from `app.rs`, which now imports it), `OpenButtons`
and two skeleton functions.

1. `choose_label(mode)`: "Open" for `OpenMode::Open`, "Import" for `Component` and `Models`.
2. `open_buttons(ui, mode) -> OpenButtons`: a `ui.horizontal` row with the confirming button
   (`look::primary_button(ui, choose_label(mode))`) followed by an ordinary `ui.button("Cancel")`.
   `chosen` / `cancelled` report this frame's clicks.
3. In `OpenDialog::show` (`app.rs`) replace the inline `ui.horizontal` row by a call to
   `open_buttons`. Behaviour stays as before: `chosen || submitted` (Enter in the path box)
   runs the existing path check (directory: enter it; file: open it; otherwise the "no such
   file" error); `cancelled` gives `DialogOutcome::Cancel`. Remove the now unused local
   `choose` label code.

## Tests

`crates/drydock/tests/open_dialog_primary.rs` (read-only): labels per mode; in both themes
and all three modes the confirming button is painted with `palette(theme).action` and
`on_action` and Cancel with neither; clicks are reported separately. Run
`harness/bin/run-tests -p drydock`. `tests/primary_buttons.rs` must still pass.

## Where to look

- `show look::primary_button`, `show look::palette`
- `show OpenDialog::show` (`outline app.rs`, the row after the path box)
- `outline open_buttons.rs`
- tests: `painted` in `tests/open_dialog_primary.rs` and `tests/primary_buttons.rs`
