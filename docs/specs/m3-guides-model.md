# m3-guides-model: guides stored in the model

Guides (`tape::Guide`, from `m3-tape`) become part of a body: undoable like any other model
change and saved in the native file. Skeletons are on main.

Change only `app/crates/modeler-kernel/src/model.rs` (the `unimplemented!` parts) and
`app/crates/modeler-kernel/src/format.rs`. Tests: `tests/guides_model.rs`,
`tests/format_guides.rs`, and two updated tests in `tests/format.rs`
(`version_is_two`, `unsupported_version`). File format: `docs/format.md` (version 2).

## Model

- `Body::guides()` (done) holds the body's guides. `Guides` equality ignores its id counter
  (done), so a model compares equal after undo even though the counter moved on.
  `Guides::insert(id, guide)` (done) puts a guide back under a given id.
- A guide is **good** when every number in it is finite and, for `Guide::Line`,
  `|dir.length() - 1| <= 1e-9`.
- Records `AddGuide(body, id, guide)` and `RemoveGuide(body, id, guide)` (inverses are done).
  Applying them, through `Model::apply` like every record (so `revision` changes):
  - `AddGuide`: `MissingBody` if the body is unknown; `BadGuide` if the guide isn't good;
    `IdInUse` if the body already has a guide with that id. Else insert it under that id.
  - `RemoveGuide`: `MissingBody`; `MissingGuide(body, id)` if there is no such guide;
    `Mismatch` if the stored guide differs (by `==`). Else remove it.
  - On error nothing changes.
- `add_guide(body, guide, log)`: checks as above (`MissingBody`, then `BadGuide`), takes the
  body's next id (never one used before in that body, even after undo or removal), applies
  and logs one `AddGuide`. Returns the id.
- `remove_guide(body, id, log)`: logs one `RemoveGuide` carrying the stored guide; returns it.
- `clear_guides(body, log)`: one `RemoveGuide` per guide in ascending id order; returns the
  removed `(id, guide)` pairs in that order. No guides: `Ok(vec![])`, nothing logged.
- Errors from the three operations leave the model and `log` unchanged.
- `Body::is_empty` counts guides (done), so `remove_definition` refuses a definition whose
  body still has guides (`DefinitionNotEmpty`).
- Guides are not geometry: they don't affect `validate`, `geometry()`, faces or edges.

Undo works with the existing `UndoStack`: push the logged edit; undo restores the model
(`==`), redo puts the same guide back under the same id.

## File

`FORMAT_VERSION` becomes 2: the reader accepts versions 1 and 2 and refuses others with
`UnsupportedVersion`. See `docs/format.md`:

- In a body, after its `instance` records, `guide` records in ascending id:
  `guide <id> line <px> <py> <pz> <dx> <dy> <dz>` or `guide <id> point <x> <y> <z>`, numbers
  printed like vertex coordinates (shortest round-trip `Display`).
- The header is `modeler-model 1` when no body has a guide, else `modeler-model 2`. So files
  without guides are byte-for-byte what version 1 wrote.
- Reading: `guide` in a version 1 file is `UnknownRecord`. In version 2: before any `body`
  record is `NoBody`; a kind other than bare `line`/`point`, a wrong token count, or a bad
  or non-finite number is `BadField`; otherwise the record is applied as `AddGuide`, and
  a refusal is `Rejected(error)` (`BadGuide` for a non-unit direction, `IdInUse` for a
  repeated id in the same body).
- Round trip: `from_text(to_text(m)) == m`, guide ids included; `to_text` of the result is the
  same text.
- `WriteError::NonFinite` if a guide has a non-finite number (can't happen through
  `add_guide`, but the writer still checks).
