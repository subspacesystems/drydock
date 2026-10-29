# m7-import-ui: File > Open and Import take STL and 3MF

Milestone M7 (3D printing). Crate `modeler`: `src/mesh_import.rs`, `src/document.rs`, `src/editor.rs`, `src/chrome.rs`, `src/viewstate.rs`, `src/app.rs`.

## Goal
STL and 3MF files open like any model (File > Open, Ctrl+O, recent files, the command line) and can be added to the open model (new File > Import...). Each solid in the file is a group. STL files do not say what their numbers mean, so the app asks for the unit (millimeters first); 3MF files carry their own. The readers are `modeler_mesh::import_stl` and `import_3mf` (on main). Skeletons are on main with `unimplemented!("m7-import-ui")` bodies; the tests are in `tests/mesh_import.rs`.

## Rules for the tested items
- `mesh_format(path)`: by extension, any letter case: `.stl` is `Stl`, `.3mf` is `ThreeMf`, anything else (including `.stl.txt`, no extension) `None`. `needs_units(path)` is true for STL only.
- `MeshUnit`: `ALL` is mm, cm, m, in in that order; `default()` is mm; `meters()` is 0.001, 0.01, 1.0, 0.0254; `label()` is `mm`, `cm`, `m`, `in`.
- `read_mesh(path, unit)`: STL goes to `modeler_mesh::import_stl(path, unit.meters())`, 3MF to `import_3mf(path)` (the unit argument is ignored: the file's own unit wins). Any other file, a missing file or a damaged one is `Err`, one line (line breaks in the reader's message become spaces), naming the file.
- `Document::open_mesh(path, unit)`: `read_mesh`, then a document like the one `Document::open` builds from a DAE file: `kind` is `FileKind::Dae` (so Save asks for a `.modeler` name), `path` is the mesh file, not untitled, not modified, scene and stats from `kernel_scene` of the model. `Document::open` sends `.stl` and `.3mf` files to `open_mesh` with the default unit (so the command line, recent files and component import work, in mm).
- `Editor::import_groups(model, source)`: everything in `source`'s root (its group instances) is added to `model`'s root at the places it has in `source`, through `copy::extract` of all root instances and `copy::insert` with the identity matrix, on the editor's own undo stack as one step labelled `Import Mesh`. Returns the number of instances in `source`'s root. A source without instances: `Err("Nothing to import")`, also kept as the editor message, nothing changed. Other kernel errors: `Err("Import failed")`.
- `units_choice(ui, file, unit)`: a label that contains `file`; four radio buttons labelled with the unit labels (selected: the one in `*unit`; clicking one sets `*unit`, and does not close the choice); buttons `OK` and `Cancel`. Returns `Some(UnitsOutcome::Ok)` / `Some(UnitsOutcome::Cancel)` on the frame the button is clicked, else `None`.
- `Action::Import` (File menu item `Import...`, between `Open...` and the separator before `Save`, no shortcut) gives `Effect::ImportDialog` and leaves the camera alone.

## App wiring (checked by hand in the real window)
- `OpenDialog` gets a mode instead of the `import` flag: Open (replaces the document), Component (the existing component import) and Models (new). Titles: "Open model", "Import component", "Import into model". The file list shows `.dae`, `.modeler`, `.stl` and `.3mf` files in every mode, and the hint text names them.
- `Effect::ImportDialog` opens the dialog in Models mode (when a document is open; otherwise ignore). Choosing a file there: for an STL, show the units choice in a modal first (default mm; remember the last choice for the session); then `read_mesh` and `Editor::import_groups` on the open document's kernel model, `refresh_document`, and show errors the way the component import does (`self.error`). A 3MF skips the units choice. The imported groups are selected afterwards if that is easy; otherwise leave the selection alone.
- Open (File > Open, recent files, Ctrl+O): an STL asks for units in the same modal, then loads through a background thread calling `Document::open_mesh(path, unit)`; a 3MF loads at once. Recent files remember the path, and reopening an STL from the recent list asks for the unit again. The unsaved-changes prompt comes before the units choice as for any open. Cancel on the units choice abandons the open or import with the model untouched.
- Components panel import keeps working and accepts the new file types (millimeters for STL, no question).
- Zoom Extents after an open shows the whole imported model.

## Acceptance
- `cargo test -p modeler --test mesh_import`
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests (existing menu tests stay green).
