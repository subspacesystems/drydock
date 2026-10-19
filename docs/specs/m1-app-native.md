# m1-app-native: open and save native files in the app

Milestone M1. Crate `app/crates/modeler`. Files: `src/document.rs`, `src/input.rs`, `src/viewstate.rs`, `src/app.rs`, `src/main.rs`. Keep public signatures and types as they are on main (the skeleton already has `FileKind`, the new `Document` fields, `Stats::faces`/`edges`, `save`, `save_as`, `native_path`, `Action::Save`/`SaveAs`, `Effect::Save`/`SaveAsDialog`); add private helpers freely. No new dependencies. No panics on any input.

Why: with this, a DAE can be converted into the kernel model and saved as a native `.modeler` file, and a native file can be opened and drawn. Tyrel tries M1 by converting a corpus model, reopening the native file and comparing.

Library pieces you use (all on main):
- `modeler_kernel::format::{load, save, FileError, ReadError, ReadErrorKind}`: read and write the native file (`save` is already crash-safe: it writes `<name>.tmp`, then renames).
- `modeler_dae::to_kernel(&dae_model) -> Result<km::Model, ConvertError>`: DAE import to kernel model.
- `modeler_dae::kernel_scene(&km::Model) -> FlatScene`: draw batches for a kernel model.
- `km::Model::root()`, `definitions()`, `body(BodyId::Definition(id))`, `Body::geometry()` (`face_count()`, `edge_count()`).

## 1. `src/document.rs` (tests: `tests/native.rs`, `tests/document.rs`)
### `native_path(path) -> PathBuf`
Extension comparisons ignore ASCII case.
- Extension `modeler`: `path` unchanged.
- Extension `dae`: replaced by `modeler` (`a/house.DAE` -> `a/house.modeler`).
- Anything else, including no extension: `.modeler` appended to the file name (`house` -> `house.modeler`, `house.v2` -> `house.v2.modeler`, `dir.d/house` -> `dir.d/house.modeler`).
- A path without a file name (`""`, `/`, `..`) is returned unchanged; saving to it then fails with an I/O error.

### `Document::open(path)`
A path whose extension is `modeler` (any case) is a native file; everything else is read as a DAE as before.
- Native: `format::load(path)`. `kind = Native`, `kernel = Some(model)`, `kernel_error = None`, `scene = kernel_scene(&model)`.
- DAE: import and `flatten` as before (`scene` is still the flattened import, so textures still draw). Then `to_kernel`: on success `kernel = Some(..)`, `kernel_error = None`; on failure the document still opens, with `kernel = None` and `kernel_error = Some(message)`. `kind = Dae`. (The DAE reader already rejects definition cycles, so no test file reaches this branch; implement it anyway.)
- `stats.triangles`, `lines`, `materials`, `size_m` come from `scene` exactly as before. `stats.faces` / `stats.edges` are the sums of `face_count()` / `edge_count()` over the root body and every definition's body (each definition once). Zero when `kernel` is `None`.

### Messages
Every error string (`open`, `save`, `save_as`, `kernel_error`) is one line (no `\r` or `\n`) and starts with the relevant file name (not the full path) followed by `": "`: the opened file for `open` and `kernel_error`, the file being written for `save`/`save_as`, and the document's file for the "nothing to save" / "not a native file" cases below. After the prefix, free text, with these requirements:
- A native file that fails to parse (`FileError::Read`) includes `line N` with the 1-based line number from the `ReadError`.
- A native file that isn't UTF-8, or doesn't start with the `modeler-model` header, gives an error (not a panic, not a DAE parse attempt).

### `save_as(path)`
- `target = native_path(path)`.
- `kernel` is `None`: `Err`, nothing written, document unchanged.
- `format::save(kernel, &target)` fails: `Err` naming `target`'s file name; document unchanged.
- Success: `self.path = target`, `self.kind = Native`; return `Ok(target)`. `scene`, `stats` and `kernel` are unchanged.

### `save()`
- `kind == Dae`: `Err` (the message says to use Save As), nothing written.
- `kind == Native`: `format::save(kernel, &self.path)`; `Ok(self.path.clone())`, or `Err` as for `save_as`. (A native document always has a kernel model; if it somehow doesn't, return `Err`.)

## 2. `src/input.rs` and `src/viewstate.rs` (tests: `tests/input.rs`, `tests/viewstate.rs`)
- `key("S", Ctrl)` -> `Action::Save`; `key("S", Ctrl+Shift)` -> `Action::SaveAs`. `S` with any other modifier set (none, Shift, Alt, Ctrl+Alt, ...) -> `None`. Other keys unchanged.
- `ViewState::apply`: `Save` -> `Effect::Save`, `SaveAs` -> `Effect::SaveAsDialog`; the camera and tool don't change.

## 3. `src/main.rs` (tests: `tests/cli.rs`, run the built binary)
- Usage text: `modeler [FILE]`, `modeler --screenshot OUT.png FILE`, `modeler --convert OUT.modeler FILE`, saying FILE is a `.dae` or `.modeler` file, and listing `--convert`.
- `--convert OUT FILE`: `Document::open(FILE)`, then `save_as(OUT)`. On success print one line `wrote <path>` (the path `save_as` returned, `Path::display`) to stdout and exit 0. On any error print `modeler: <message>` to stderr and exit 1 (including a DAE whose `kernel` is `None`).
- `--convert` without its OUT argument, `--convert` without FILE, or `--convert` together with `--screenshot`: usage error on stderr, exit 2, nothing written.
- `--screenshot` works for `.modeler` files too (it goes through `Document::open`).

## 4. Window (`src/app.rs`; no automated tests, reviewed by running it)
- Keep the open `Document` in the app (not only its path and stats) so it can be saved.
- Open dialog and drag and drop accept `.modeler` files as well as `.dae`. The dialog lists both (folders first, then files, by name as now).
- `Effect::Save`: no document or loading in progress -> nothing. Native document -> `save()`. DAE document -> the Save dialog, as for Save As.
- `Effect::SaveAsDialog` (no document -> nothing): a modal in plain egui like the open dialog: folder browser starting in the document's folder (fallback: current directory), listing folders and `.modeler` files, with a file name field prefilled with the file name of `native_path(document.path)`. Save button or Enter -> target `native_path(folder.join(name))`; if that file already exists, ask "Replace <name>?" (Replace / Cancel) first. Then `save_as`. Cancel or Escape closes. An empty name does nothing.
- After a save: status bar shows "Saved <file name>" until the next open or save; window title from `Document::title()` (a DAE saved as native now shows the `.modeler` name). On error: the one-line message in the status bar in the error color, document unchanged.
- Status bar: after materials, "<faces> faces, <edges> edges" when the document has a kernel model; when `kernel_error` is set, show it in the error color (the model is still drawn).
- Saving runs on the UI thread (it's fast enough for corpus models); keyboard input is ignored while the dialog is open, as now.
- No names, icons or art from other modeling products. Plain egui widgets only.

## Tests (fixed; don't edit)
- `tests/native.rs`: `native_path` cases; opening DAE and native fixtures (kind, kernel, stats, title, scene from `kernel_scene`); upper-case extension; broken native files (bad header, bad record with line number, not UTF-8, missing file); `save_as` from DAE and reopening (same model text, same size and counts); `save` on native and DAE documents; write failures; components in `tables.dae` (definitions counted once).
- `tests/cli.rs`: `--convert` success, failures and usage errors; `--help`; `--screenshot` of a `.modeler` file (renders on the oracle's software GPU).
- `tests/input.rs`, `tests/viewstate.rs`: the Save key bindings and effects.
- `tests/fixtures/slab.modeler`: `slab.dae` converted by `to_kernel` and written by `format::to_text`.
