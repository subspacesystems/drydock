# m10-model-source-trait: the ModelSource interface and the local-file source

New module `app/crates/drydock/src/model_source.rs` (skeleton on main: types and trait are
real, the bodies marked `unimplemented!("m10-model-source-trait")` are yours). Tests:
`app/crates/drydock/tests/model_source.rs`. Do not change any call site in the app; wiring
the Import dialog and component import through this is `m10-model-source-wire`.

## What it is

A `ModelSource` lists models for a search text and fetches one as bytes plus a file name and
format. `LocalFileSource` is the first source: the model files in one folder. An online
library can be added later as another `ModelSource`.

## Behaviour to implement

- **`SourceFormat::from_path`**: by extension, any letter case: `dae` Dae, `stl` Stl, `3mf`
  ThreeMf, `drydock` and `modeler` Native; anything else (or no extension) `None`.
- **`LocalFileSource::name`**: a short non-blank name, e.g. `"Local files"`.
- **`list(query)`**:
  - Looks at the files directly in the folder (not subfolders, not directories) whose
    `SourceFormat::from_path` is `Some`.
  - `title` is the file name without its extension; `id` is the file's full path
    (`dir.join(file_name)` as a string); `format` is `Some(format)`.
  - A blank query (empty or whitespace) keeps everything; otherwise the trimmed query must be
    a case-insensitive substring of the title. It never matches the extension or the folder.
  - Sorted by title, case-insensitively, ties by id.
  - A folder that can't be read: `Err` with one line (no `\r`/`\n`) that names the folder.
- **`fetch(entry)`**: `entry.id` is a path. Format from its extension, else `Err` naming the
  file (e.g. `notes.txt: not a model file`). Reads the bytes; a read error is `Err` naming the
  file. Result: `file_name` (last path component), `bytes`, `format`, `origin: Some(path)`.
  One-line errors.
- **`SourceModel::title`**: `file_name` without its extension.
- **`SourceModel::into_model(unit)`**: picks the reader by `format`.
  - Dae and Native: `components_panel::read_component_source(path)` (its model).
  - Stl and ThreeMf: `mesh_import::read_mesh(path, unit)`; `unit` matters for STL only.
  - `path` is `origin` when set. Otherwise create a fresh folder under
    `std::env::temp_dir()` named `drydock-source-<pid>-<counter>`, write `bytes` to it as
    `file_name` (last component only), read it, and remove the folder again on success and on
    error. Errors name the file and have no line breaks.
- Do not use a format's own error text if it lacks the file name: prefix `file_name: `.

## Documentation

The module docs already say: synchronous, an online source is called from a worker thread
(`Send + Sync`), and `list` matches titles and tags only, never the contents of models.
Keep those statements; do not weaken them.

## Where to look

- `show read_component_source` (components_panel.rs), `show read_mesh`, `show mesh_format`
  (mesh_import.rs), `show is_native_file` (document.rs).
- `show Document::open` for what the Dae/Native readers do.
- Test helpers in `tests/model_source.rs`: `library()`, `temp_dir`, `fixture`; fixtures in
  `app/crates/drydock/tests/fixtures/` (`slab.dae`, `slab.drydock`, `slab.modeler`).
