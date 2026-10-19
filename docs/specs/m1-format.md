# m1-format: native model file

Milestone M1, kernel task. Crate `app/crates/modeler-kernel`, file `src/format.rs`. Implement every `unimplemented!("m1-format")`. The file format is defined in `docs/format.md`; read it first, it is part of this spec. Don't change public signatures or public types; add private helpers freely. Don't edit `geom.rs`, `topology.rs`, `model.rs` or `sticky.rs`. Dependencies: `std` and `glam` only. No panics on any input.

## to_text
- Writes exactly what "What writers produce" in `docs/format.md` describes, using only the public API of `Model`, `Body` and `Context`.
- Numbers: `f64` and `f32` with `{}` (`Display`); ids with `{}`. Strings always quoted and escaped as described.
- Paint lines only for face material entries whose face exists in that body's geometry.
- Returns `WriteError::NonFinite` if any vertex coordinate, material color component or instance transform entry is NaN or infinite.

## from_text
Reads line by line (`str::lines`, so `\r\n` works). Line numbers are 1-based. The first problem found, in reading order, is returned as `ReadError { line, kind }`; checks within a line run in the order listed here.

1. Line 1 must split into exactly two tokens `modeler-model` and a `u32`; otherwise (including empty text) `NotAModelFile` at line 1. A version other than `FORMAT_VERSION` is `UnsupportedVersion(v)` at line 1.
2. Blank and `#` lines are skipped. Tokenize the line: a token beginning with `"` is a quoted string; a bad escape, a missing closing quote, or a closing quote followed by anything other than a space, tab or end of line is `BadField`. Bare tokens never contain `"` (a `"` inside a bare token is `BadField`).
3. The first token names the record. A quoted or unknown name is `UnknownRecord`.
4. Field count and each field: wrong token count, a quoted token where a bare one is expected or the reverse, an id that isn't a `u64` (`str::parse::<u64>`), a number that doesn't parse (`str::parse::<f64>`, `f32` for colors) or isn't finite, a `kind` other than `component`/`group`, a body other than `root`/an id, an edge use other than `+<u64>`/`-<u64>` -> `BadField`. Field counts: material 8, definition 4, body 2, vertex 5, edge 4, face at least 2, paint 4, instance 21, end 1. In a `face` line every token after the id is an edge use or a bare `|`.
5. `vertex`, `edge`, `face`, `paint`, `instance` before any `body` record -> `NoBody`. A `body` record for a body already seen (root or the same definition id) -> `DuplicateBody`.
6. Apply the record to the model being built and map a `ModelError` to `Rejected(e)`:
   - `material` -> `ModelRecord::AddMaterial`; `definition` -> `AddDefinition` (`group` sets `DefinitionInfo::group`).
   - `body <id>`: the definition must exist, else `Rejected(ModelError::MissingBody(BodyId::Definition(id)))`. `body root` always works.
   - `vertex`, `edge`, `face` -> `ModelRecord::Geometry(current body, Record::Add*)`.
   - `paint`: the face must exist in the current body's geometry, else `Rejected(ModelError::Topo(TopoError::MissingFace(f)))`; then `SetFaceMaterials` with `from` the current value (a second paint of the same face replaces the first; `- -` clears it).
   - `instance` -> `AddInstance(current body, id, ...)` with the transform `DMat4::from_cols_array(&m)`.
7. `end` stops reading. Any later line that isn't blank -> `TrailingData` at that line. If there is no `end` -> `MissingEnd` with `line` = the number of lines in the text.

Id counters come out of `Model::apply` (one above the highest id read).

## save and load
- `save(model, path)`: `to_text` first (`Write(e)` on error, nothing touched on disk). The temporary file is `path` with `.tmp` appended to its file name, in the same directory; a `path` without a file name is `Io` with kind `InvalidInput`. Create or truncate the temporary file, write the text, `sync_all`, close, then `std::fs::rename` it to `path`. On Unix, then open the parent directory and `sync_all` it, ignoring any error. If any step before the rename completes fails, remove the temporary file (ignore errors) and return `Io { kind, message }` (`message` is the error's `to_string()`).
- `load(path)`: read the bytes (`Io` on error), `NotUtf8` if not UTF-8, then `from_text` (`Read(e)`).

## Round trip
For any model `m` with finite numbers: `from_text(&to_text(&m)?)` equals `m` once `m`'s face material entries for missing faces are dropped, and writing the result again gives the same text.

## Tests (fixed; don't edit)
`tests/format.rs` (exact writer output, every record, string escapes, each error kind and its line, save/load on disk) and `tests/format_props.rs` (proptest: random models round-trip exactly and rewrite to the same text; damaged text never panics).
