# m8-rename-extension: native files are `.drydock`

Native files are saved as `.drydock` with the first line `drydock-model <version>`. Files with the
older `.modeler` extension and the older `modeler-model` first line keep opening. Tyrel's decision: no file association for `.modeler`; the installer (a later task) registers only `.drydock`.

The format **version does not change**: only the header word differs, so a model that needed
version 4 before is still version 4. Nothing to bump.

## 1. `modeler_kernel::format` (`format.rs`)
- `to_text` writes the first line `drydock-model <version>` (everything after it is unchanged).
- `from_text` accepts the word `drydock-model` or `modeler-model`, then the version as before.
  Any other first word, a missing or extra token, a non-numeric version, or a header that is not
  on line 1 is still `ReadErrorKind::NotAModelFile` at line 1; versions outside `1..=FORMAT_VERSION`
  are still `UnsupportedVersion`. The words are matched exactly (case-sensitive).
- Update the doc comment on `ReadErrorKind::NotAModelFile`.
- Saving a model that was read from an old-header file writes the new header.

## 2. `modeler::document` (`document.rs`)
The skeleton on main has `NATIVE_EXTENSION` (`"drydock"`), `OPEN_EXTENSIONS` (the Open dialog's
list) and two stubs to fill:
- `is_native_file(path)`: extension is `drydock` or `modeler`, any case.
- `is_openable_file(path)`: extension is one of `OPEN_EXTENSIONS`, any case.
- `Document::open` takes the native route when `is_native_file(path)` (it tested `modeler` only).
- `native_path(path)`: a native extension (either) is kept as given, `.dae` (any case) is replaced
  by `.drydock`, anything else gets `.drydock` appended (`house.v2` -> `house.v2.drydock`); a path
  without a file name is unchanged. So Save on an opened `.modeler` file keeps its path.
- `Document::new_empty` is named `untitled.drydock`.
- The `NotAModelFile` message becomes `not a Drydock model file` (it is shown as
  `<file>: line 1: not a Drydock model file`). It must not say `modeler`.
- Update the doc comments that mention `.modeler`.

## 3. `app.rs` and `main.rs`
- Open dialog `enter`: use `is_openable_file` instead of the inline list.
- Save As dialog `enter`: `SaveMode::Model` filters on `NATIVE_EXTENSION`.
- The default Save As name fallback `model.modeler` becomes `model.drydock`.
- `main.rs` help text: `--convert OUT.drydock`, `FILE is a .dae, .drydock or .modeler file`, and
  "native .drydock file". Leave the program name and other words to `m8-rename-text`.

## 4. `docs/format.md`
Document the new header `drydock-model <version>`, that readers also accept the older
`modeler-model <version>`, that writers always write the new one, and that the version number is
unchanged by the rename. Native files are `.drydock` (older `.modeler` still open).

## Tests (already written, must pass; do not edit)
- `modeler-kernel/tests/header.rs`: written header, old header reads and rewrites, versions, bad
  first lines.
- `modeler/tests/native_ext.rs`: extension helpers, `native_path` table, `.dae` -> `.drydock`,
  Save on an opened `.modeler` keeps its path and writes the new header, case-insensitive open,
  error message.
- Existing tests that pinned the old header or extension (`modeler-kernel/tests/*.rs`,
  `modeler/tests/native.rs`, `cli.rs`, `document_edit.rs`) were updated; a new fixture
  `tests/fixtures/slab.drydock` sits beside `slab.modeler` (old header, still used for reading).

## Where to look
`show to_text`, `show from_text`, `show ReadErrorKind`, `show native_path`, `show Document::open`,
`show Document::new_empty`, `show file_format_error`, `show is_native_file`,
`show is_openable_file`; in `app.rs` the two `fn enter` of the Open and Save dialogs
(`rg "SaveMode::Model"`) and the `native_path(&document.path)` call near `model.modeler`;
`usage()` in `main.rs`; test helpers in `modeler/tests/native.rs` (`open`, `temp_dir`).
