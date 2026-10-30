# m7-export-ui: File > Export 3MF... and Export STL...

Milestone M7 (3D printing). Crate `modeler`: new `src/mesh_export.rs`, plus `src/app.rs`, `src/chrome.rs`, `src/viewstate.rs`, `src/input.rs`.

## Goal
Two File menu items write the model's solids to a print file: 3MF (one file) or STL (one file, or one per object). A dialog first asks what to export and how. The mesh comes from `modeler_mesh::print_mesh::print_mesh` (solid groups and components as world-space triangles in mm, curves at a chord tolerance, non-solid objects skipped with a reason); the writers are `modeler_mesh::threemf_write::write`, `stl_write::write_combined` and `write_separate`. Skeleton, `Action::ExportMesh(ExportFormat)`, `Effect::ExportDialog(ExportFormat)`, `MenuState::can_export` and the two menu items are already on main; `mesh_export.rs` bodies are `unimplemented!("m7-export-ui")`. Tests: `tests/mesh_export.rs`.

All logic lives outside egui: `export` is a plain function from model, selection and options to files, so most tests are plain unit tests.

## Rules for the tested items
- `parse_tolerance_mm(text)`: trim; drop one trailing `mm` and trim again; parse as `f64`; accept only finite values in `MIN_TOLERANCE_MM..=MAX_TOLERANCE_MM`. Anything else (empty, `NaN`, `inf`, other units, two numbers, a decimal comma) is `None`.
- `export(model, selection, options, path)`, in this order:
  1. `options.tolerance_mm` outside the same range (or NaN): `Err` mentioning "tolerance"; nothing written.
  2. Scope: whole model = `PrintScope::Model` (the selection is ignored). `selection_only` = `PrintScope::Instances` of the `Entity::Instance` entries of `selection`; with none: `Err("Nothing selected")`.
  3. `print_mesh` with the tolerance. For STL, scale every corner by `1 / mm-per-unit` of `options.unit` (mm 1, cm 10, m 1000, in 25.4) before writing; 3MF is always millimeters and ignores `unit` and `layout`.
  4. No object with triangles: `Err` starting `Nothing to export`, followed by `: ` and the skipped objects as `<name> <reason>` (the reason's `Display`, e.g. "Leaky is not a solid: 1 open edge") joined by `; `. For an empty model nothing follows the words (`Nothing to export`). No file or folder is created.
  5. Write. 3MF and single STL: `path` gets the format's extension appended when it lacks it (compared ignoring case; `solid.part` becomes `solid.part.stl`, `LOUD.3MF` stays). `StlLayout::OneFilePerObject`: `path` is the folder (created by the writer), no extension added.
  6. A writer error becomes `Err("<path>: <writer message>")`, one line.
  The report: `files` as written, `objects` and `triangles` counts of what was written (after skipping), `skipped` from `print_mesh`.
- `status_message(report, options)`: `Exported N object(s) (T triangles) to <file name>`; for one file per object `... as K files in <folder name>`; then `; skipped <name> <reason>` for each skipped object. `object` is singular only for N = 1; the word `triangles` is always plural. File and folder names are the last path component only.
- `ExportDialog::new(options)` (already written) shows `options`; the tolerance text starts as the number as Rust prints it (`0.01`, `0.05`).
- `export_dialog(ui, dialog, has_selection)`:
  - Radios `Whole model` and `Selection` (sets `selection_only`). `Selection` is disabled when `has_selection` is false; when it is false and `selection_only` is already true, the dialog resets it to false.
  - STL only: radios `mm`, `cm`, `m`, `in` (`MeshUnit::label`, sets `unit`) and radios `One file per object` and `Single file` (sets `layout`). A 3MF dialog shows none of these labels.
  - A label `Curve tolerance (mm)` and one single-line text box bound to `tolerance_text`. Every frame the text parses with `parse_tolerance_mm`, `options.tolerance_mm` is set from it.
  - Buttons `OK` and `Cancel`. `OK` is disabled while the text does not parse. Returns `Some(ExportOutcome::Ok)` / `Some(ExportOutcome::Cancel)` on the frame a button is clicked, else `None`.
- Menu: `Export 3MF...` and `Export STL...` (done) sit between `Import...` and `Save`; they are disabled when `MenuState::can_export` is false. Set `can_export` in `App::menu_state` (`app.rs`, currently `false`) to: the model's root holds at least one instance.

## App wiring (checked by hand in the real window)
- Replace the placeholder arm `Effect::ExportDialog(_format)` in `app.rs` (next to `Effect::ImportDialog`): ignore when no document is open; otherwise open a modal window "Export 3MF" / "Export STL" showing `export_dialog` with an `ExportDialog` built from the options of the last export in this session (`ExportOptions::new(format)` the first time, remembered per format), `has_selection` = the editor's selection holds an instance. Mirror how the units modal for imports (`UnitsPurpose`, `units_choice`) is shown.
- `OK` closes the modal and opens the existing save dialog (`open_save_dialog` and its mode machinery) for the file name, starting with the document's name and the format's extension. For `OneFilePerObject` the same dialog asks for a folder name: take the typed name without its extension as the folder, created next to where the dialog is. Choosing a name runs `export` on the open document's kernel model with the editor's selection; `Ok(report)`: show `status_message` in the status bar (the same place as other notes, e.g. the fit check note) until the next action; `Err(message)`: show it as `self.error`. Cancel anywhere abandons the export; the model is untouched and not marked modified.
- Remember the options chosen at `OK` for the next export of that format.
- Exporting never changes the document, its path or its modified flag.

## Acceptance
- `cargo test -p modeler --test mesh_export`
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
- By hand in the window: File > Export STL... on two solid groups writes files that open again through File > Import... at the same size; a model with an open box shows the skipped name in the status bar.

## Where to look
`harness/bin/show` these: `print_mesh` and `PrintScope`/`PrintOptions`/`PrintMesh`/`Skipped`/`SkipReason` (`modeler-mesh/src/print_mesh.rs`), `threemf_write::write`, `stl_write::write_combined`, `stl_write::write_separate`, `MeshUnit` and `units_choice` (`mesh_import.rs`, the model for the dialog and its tests), `Effect::ImportDialog` handling and `UnitsPurpose` in `app.rs`, `open_save_dialog`, `App::menu_state`, `Editor::selection`, `Model::root`. Test helpers: `model_of`, `file_menu`, `dialog_harness` in `tests/mesh_export.rs`.
