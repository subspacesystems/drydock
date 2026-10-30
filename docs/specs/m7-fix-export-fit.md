# m7-fix-export-fit: sign-off fixes for export and the fit tint

Three small bugs found in the M7 sign-off. The rules for the first two are pure functions in
`mesh_export.rs` (skeletons on main, `unimplemented!()`); you fill them in and wire all three
into `app.rs`. Tests: `modeler/tests/mesh_export_fix.rs` (do not edit).

## 1. File > Export is always greyed out
`MenuState::can_export` is hard-coded `false` in the MenuState built in `app.rs`.

- Fill in `mesh_export::can_export(model)`: true when the root body has at least one instance
  (group or component). Loose geometry alone is false.
- In `app.rs` set `can_export` from the open document's kernel model:
  `self.model.as_ref().and_then(|d| d.kernel.as_ref()).is_some_and(mesh_export::can_export)`.

## 2. Export file name and the "Replace?" question
`open_export_save_dialog` builds the suggested name from `native_path(&document.path)`, so a
document `two.stl` suggests `two.stl.stl`.

- Fill in `mesh_export::default_export_name(document_path, format, layout)`: the file name of
  `document_path` with its last extension removed (`two.stl` -> `two`, `a.modeler` -> `a`,
  `x.dae` -> `x`, `part.v2.stl` -> `part.v2`); `untitled` when the path has no file name.
  Then add `.stl` or `.3mf`. For `ExportFormat::Stl` with `StlLayout::OneFilePerObject`
  (a folder) return just the stem. Use it in `open_export_save_dialog` instead of
  `native_path(..).file_stem()`.
- `SaveDialog::show` asks "Replace X?" whenever the target exists. In `SaveMode::ExportFolder`
  an existing directory is fine (the export only creates files in it), so skip the question when
  the mode is `ExportFolder` and the target `is_dir()`. Keep the question for a single-file
  export onto an existing file, and for the model save.

## 3. Redraw after the fit tint changes
`show_scene` uploads the scene but never sets `self.dirty = true`, so the Fit Check tint
(turned on, off, or recomputed) shows only after the camera moves. Set `self.dirty = true`
at the end of `show_scene` (after the upload; also on its early returns after `self.fit`
was cleared is fine). This is checked by hand in the real window after merge; there is no
automated test.

Do not touch other behaviour. `cargo fmt`, clippy `-D warnings` and the existing tests must stay
green.

## Where to look
- `mesh_export::export`, `ExportOptions`, `ExportFormat::extension`, `StlLayout`
- `app.rs`: `open_export_save_dialog`, `SaveDialog::show` / `target` / `enter`, `SaveMode`,
  `show_scene`, the `MenuState { .. can_export .. }` initializer
- `document::native_path` (what not to use for the export name)
- `chrome::MenuState::can_export`, `chrome::menu_bar`
- `Model::root`, `Body::instances`
- `fit_check::display_scene`; test helper `file_menu` in `modeler/tests/mesh_export.rs`
