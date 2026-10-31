# m7-fix-dialogs: Enter presses OK; Print Setup opens below the menus

Found in the M7 sign-off. The M6 dialog rule (`docs/specs/m6-dialogs.md`: one default button, Enter presses it when enabled, Escape is the caller's) is not followed by three M7 dialogs, and Print Setup covers the menu bar.

## Changes
- `mesh_export::export_dialog`, `mesh_import::units_choice` and `text_tool::text_dialog`: replace the hand-made `ui.horizontal` button pair with `dialog::button_row`, buttons `OK` (default; enabled as now) and `Cancel` (not default). Map index 0 to the OK outcome, 1 to Cancel. Outcomes, labels and everything above the buttons stay as they are.
  - Export: OK is enabled only while the tolerance parses, so Enter does nothing otherwise.
  - 3D Text: OK is enabled only while `dialog.options()` is `Some` (empty text is `None`), so Enter does nothing then.
- `print_setup::window_pos(available)`: return `available.min + vec2(WINDOW_MARGIN, WINDOW_MARGIN)`. The skeleton is already wired in `app.rs` as the Print Setup window's `default_pos` (with the central panel's rect, the area left after the menu bar and toolbars). Do not change `app.rs`.

## Tests
`modeler/tests/dialogs_enter.rs` (given, do not edit): Enter returns OK in all three dialogs, does nothing while OK is disabled, Cancel and the OK button are unchanged; `window_pos` is the corner plus margin; a Print Setup window placed with it sits below a top panel.

## Where to look
- `dialog::button_row`, `DialogButton` (`modeler/src/dialog.rs`); `dialog::confirm_dialog` as an example caller.
- `mesh_export::export_dialog`, `ExportOutcome`; `mesh_import::units_choice`, `UnitsOutcome`; `text_tool::text_dialog`, `TextOutcome`, `TextDialog::options`.
- `print_setup::window_pos`, `WINDOW_MARGIN`.
- Existing kittests: `tests/mesh_export.rs`, `tests/mesh_import.rs`, `tests/text_tool_ui.rs`, `tests/print_setup_ui.rs` must keep passing.
