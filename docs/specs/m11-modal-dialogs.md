# m11-modal-dialogs: the remaining prompts open as modal windows

Milestone M11. `m11-modal-window` added `dialog::modal_dialog_window` and moved
the shared prompts to it. This task moves the rest of the small prompts: unsaved changes, mesh
units, 3D Text, export options and Purge Unused. Our own Open and Save dialogs follow in
`m11-modal-file-dialogs` (they are big, and the system file picker replaces them where it
exists); do not touch them here.

## What is already on main

- `dialog::modal_dialog_window(ctx, id, &DialogWindow { title, size }, &mut open, content)`
  (after `m11-modal-window`): a real OS window marked modal, or an `egui::Modal` with the
  title line when viewports are embedded (tests). Escape and the title bar's close clear
  `open`. Do not draw the title yourself inside `content`; the helper does.
- Skeleton `src/prompts.rs` with `UnsavedChoice` and four `unimplemented!` functions:
  `unsaved_prompt`, `units_prompt`, `text_prompt`, `export_prompt`. Their doc comments give
  titles and sizes.

## What to build

1. **Fill in `prompts.rs`.** Each function calls `modal_dialog_window` with a fixed id and
   size, keeps a local `open` flag set to `true` per call, runs the existing content function
   (`mesh_import::units_choice`, `text_tool::text_dialog`, `mesh_export::export_dialog`; unsaved
   draws its line and three buttons as `unsaved_dialog` in `app.rs` does today, Save through
   `look::primary_button`) and returns the answer. If the window closed (`!open`) with no
   answer, return Cancel (`UnsavedChoice::Cancel`, `UnitsOutcome::Cancel`, `TextOutcome::Cancel`,
   `ExportOutcome::Cancel`). The content must not draw a heading any more: the title is the
   window title.
2. **`app.rs`.** Replace the four `egui::Modal` blocks (mesh units, 3D Text, export options,
   and `unsaved_dialog`) with calls to these functions; delete `unsaved_dialog` and the private
   `UnsavedChoice` (use `prompts::UnsavedChoice`). Keep every outcome's handling as it is.
   `dialog_open` (the check that keeps global keys away from the main window) still lists all
   four prompts. Do not move other code; app.rs is large, read only these ranges
   (`show units_prompt` style lookups: `grep -n "units_prompt\|text_dialog\|export_dialog\|unsaved_dialog" app.rs`).
3. **Purge Unused** (`materials_panel.rs`, the `panel.purge_confirm` block): use
   `modal_dialog_window` with title "Purge Unused Materials" (not "Purge Unused": that is the
   button's label and tests find buttons by label), id `purge-unused`, about 380 x 300. Same
   content, buttons and results; closing without an answer (Escape, title bar) clears
   `purge_confirm`. `tests/materials_purge_confirm.rs` passes unchanged.
4. Nothing else changes: same texts, same buttons, same defaults and Enter behaviour
   (`tests/dialogs.rs`, `tests/dialogs_enter.rs`, `tests/text_tool_ui.rs`, `tests/mesh_export*.rs`,
   `tests/units_ui.rs` pass unchanged).

## Acceptance tests (do not edit)

`tests/modal_prompts.rs` (kittest, embedded): each prompt shows its window title and content;
buttons give the right answer and keep edited state (chosen unit, text mode); Escape is Cancel;
a click on a main-window button does nothing while a prompt is open; Purge Unused has its own
title, blocks the main window, Cancel closes it, Purge sends one `MaterialAction::Purge`.

Run `harness/bin/run-tests -p drydock` at the end (Xvfb, `MODELER_GUI_TESTS=1`).

## Where to look

- `show modal_dialog_window`, `show DialogWindow`, `show confirm_dialog` (a finished example
  of a prompt built on it), `show button_row`, `show dialog_title`.
- `show units_choice`, `show text_dialog`, `show export_dialog`, `show look::primary_button`.
- `show unsaved_dialog` and the blocks that call it, `UnitsPrompt`, `TextPrompt`, `ExportPrompt`
  in `app.rs` (`outline src/app.rs` first).
- `show materials_panel` (the `purge_confirm` block near its end), `MaterialsPanel::confirming`.
- Tests: `tests/modal_window.rs` (harness pattern), `tests/materials_purge_confirm.rs`.
