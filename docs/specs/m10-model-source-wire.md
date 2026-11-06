# m10-model-source-wire: import through a ModelSource

Skeleton on main: `app/crates/drydock/src/model_source.rs` has six new public functions with
`unimplemented!("m10-model-source-wire")` bodies. Tests:
`app/crates/drydock/tests/model_source_wire.rs` (a fake in-memory source plus the local
source). You fill the six bodies and change the two import methods in `app.rs`.

## Goal

The Import dialog (components, and Insert > Model meshes) and the component import must get
the file's bytes and name from a `ModelSource` (the `m10-model-source-trait` code) instead of
reading the file themselves, so an online library can be added later as another source.
Nothing changes for the user: same dialog, same names, same messages, no network code.

## The six functions (in `model_source.rs`)

- **`entry_for_path(path)`**: `SourceEntry { id: path as string, title: file name without
  extension, format: SourceFormat::from_path(path) }`.
- **`local_source_for(path)`**: `LocalFileSource::new(parent folder)`; `.` when `path` has no
  folder part (`Path::new("a.dae").parent()` is the empty path).
- **`fetch_component(source, entry)`**: `source.fetch(entry)?` (its `Err` is returned as is),
  then `into_model(MeshUnit::default())`; returns the model with the name `title()`.
  Every `SourceFormat` is accepted (as `Document::open` accepts STL and 3MF today).
- **`fetch_meshes(source, entry, unit)`**: fetch (`Err` as is). If the fetched `format` is
  not `Stl` or `ThreeMf`: `Err(format!("{file_name}: not an STL or 3MF file"))` and nothing
  is read. Otherwise `into_model(unit)`.
- **`import_component_from(editor, model, source, entry)`**: `fetch_component`, then
  `editor.import_component(model, &source_model, &name)`. Any `Err` leaves `model` unchanged.
- **`import_meshes_from(editor, model, source, entry, unit)`**: `fetch_meshes`, then
  `editor.import_groups`. Returns its count. Any `Err` leaves `model` unchanged.

## The app

- `App::import_component_file(ctx, frame, path)`: build `local_source_for(path)` and
  `entry_for_path(path)` and call `import_component_from` on the open document's kernel;
  keep the existing handling of "no open model" / kernel error and `refresh_document`.
- `App::import_mesh_file(ctx, frame, path, unit)`: the same with `import_meshes_from`.
- Neither method may call `read_component_source` or `mesh_import::read_mesh` any more
  (`SourceModel::into_model` does). Leave `Document::open`, `open_mesh`, `needs_units`, the
  dialog and the units prompt alone: opening a file is not part of this task.
- Errors keep going to `self.error` as one line naming the file. A file with an extension that
  is no model format now reports `<name>: not a model file`; the dialogs filter such files, so
  this is only a corner case.

## Do not

- No network code, no new dependency, no new UI, no source picker yet.
- Don't change `ModelSource`, `SourceModel` or `LocalFileSource` behavior, or existing tests.

## Where to look

`show` these: `ModelSource`, `LocalFileSource`, `SourceModel::into_model`,
`SourceModel::title`, `SourceFormat::from_path`, `Editor::import_component`,
`Editor::import_groups`, `App::import_component_file`, `App::import_mesh_file`,
`read_component_source`, `read_mesh`. Test helpers: `MemorySource`, `fixture`, `temp_dir`
in `tests/model_source_wire.rs`; the existing `tests/model_source.rs`,
`tests/components_import_ui.rs` and `tests/mesh_import.rs` must still pass.
