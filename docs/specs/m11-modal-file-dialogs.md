# m11-modal-file-dialogs: our Open and Save dialogs as modal windows

Milestone M11. `m11-modal-window` added `dialog::modal_dialog_window`;
`m11-modal-dialogs` moved the small prompts to it (`src/prompts.rs`). This task does the same
for the app's own Open and Save dialogs, now in `src/file_dialogs.rs` (already moved out of
`app.rs`, unchanged). They stay as the fallback where no system file picker exists
(`m11-native-file-dialogs`), so they must work well. Do not touch `app.rs` unless a call site
fails to compile.

## What to build

1. **`OpenDialog::show`** draws through `modal_dialog_window` (id `open-dialog`,
   `DialogWindow { title, size: 560 x 480 }`). The title is the window title and no longer a
   `ui.heading`: `Open model` (`OpenMode::Open`), `Import component` (`Component`),
   `Import into model` (`Models`). The rest of the content is as today (folder line,
   scrolling list, path field, buttons).
2. **`SaveDialog::show`** likewise (id `save-dialog`, 560 x 480). Titles: `Save model`,
   `Export {format label}` (`Export STL`, `Export 3MF`), `Export {format label} to folder`.
3. **Closing without an answer** (Escape, the title bar's close) is Cancel
   (`DialogOutcome::Cancel`, `SaveOutcome::Cancel`), whatever the dialog was showing, the
   replace question included. Use a local `open` flag set to `true` per call, as
   `prompts.rs` does; if it is cleared and the outcome is still `Pending`, return Cancel.
4. **Unchanged:** the `show(&mut self, &egui::Context) -> Outcome` signatures, Enter in the
   path/name field, the Replace question and its Cancel (back to the name), error text,
   primary button via `look::primary_button` / `open_buttons`. The title can't be found twice
   in the tree, so the title must not be drawn inside the content as well.
5. The size is the window's initial size; the list's `max_height(320.0)` may be adjusted so
   it fits 560 x 480 without a second scroll bar.

`tests/open_dialog_primary.rs`, `tests/dialogs.rs`, `tests/dialogs_enter.rs`,
`tests/mesh_export*.rs` and `tests/viewstate.rs` pass unchanged.

## Acceptance tests (do not edit)

`tests/modal_file_dialogs.rs` (kittest, embedded): titles per mode appear once; the list shows
folders and model files; picking a file gives `Open(path)`; the main window ignores a click
while a dialog is open; Escape and Cancel give Cancel; entering a folder keeps the dialog
open; Save with a new name gives the `.drydock` path; an existing name asks
`Replace house.drydock?`, Replace saves, Cancel goes back to the name field.

Run `harness/bin/run-tests -p drydock` at the end (Xvfb, `MODELER_GUI_TESTS=1`).

## Where to look

- `show OpenDialog::show`, `show SaveDialog::show`, `outline src/file_dialogs.rs`.
- `show modal_dialog_window`, `show DialogWindow`; `src/prompts.rs` (`show unsaved_prompt`,
  `show export_prompt`) for the local-`open`-flag pattern.
- `show open_buttons`, `show look::primary_button`, `show ExportOptions::new`.
- Tests: `tests/modal_prompts.rs` (same harness idea), `tests/open_dialog_primary.rs`.
