# m10-import-component-tolerant: import components that already have geometry problems

Skeleton on main: `drydock::model_source::import_failure_message` (returns the old
"Import failed" until you fill it in), and `Editor::import_component` already calls it for
every refusal except `CopyError::Empty`. Tests:
`app/crates/drydock-kernel/tests/import_component_tolerant.rs` (kernel) and
`app/crates/drydock/tests/import_component_tolerant_ui.rs` (the message and the editor).
You change `import_component` and `copy_body_into` in
`app/crates/drydock-kernel/src/copy.rs` and fill `import_failure_message` in
`app/crates/drydock/src/model_source.rs`. Don't touch `editor.rs` or `app.rs`.

## Goal

Many models made in other programs have small geometry faults, most often two edges that
cross without a shared vertex (`Violation::EdgesIntersect`). Such a file opens fine (opening
doesn't validate), but Components > Import refuses it with "Import failed":
`copy::import_component` validates the model after the copy and refuses every violation that
wasn't there before, and the source's own faults are "new" in the target model because they
now live in new definitions.

After this task an import is refused only for problems the import itself adds. A source's
own faults come along unchanged, in the copies of the bodies they were in. When an import is
still refused, the status bar says why in one line.

## Kernel: `copy::import_component`

Keep everything it does today (extract the source's root into a clipboard, copy materials
and definitions, the new component definition, one undo step labelled `label`, the model
and the undo stack unchanged on every `Err`, `source` unchanged). Change only which
violations count as added:

1. **Carried violations.** Right after `extract`, take `clip.validate()` (the clipboard
   model `clip = clipboard.model()`, whose definitions keep the source's ids). These are the
   faults the import is allowed to carry.
2. **Remember the id maps.** `copy_body_into` returns, instead of `()`, the maps it built:
   source vertex, edge, face, curve and instance id to the new id in the target body. Use
   a private struct (e.g. `BodyIds`) with one `BTreeMap` per kind. Curves: `curve::carry`
   returns one `Option<CurveId>` per curve it was given, in order; pair them with the ids of
   the source curves taken in the same order, and skip the `None`s. Instances:
   `Model::add_instance` returns the new id. The two other callers of `copy_body_into`
   (`insert`) ignore the result.
3. **Body map.** While copying, record for each clip body where it went:
   `BodyId::Definition(d)` to `BodyId::Definition(definition_map[d])`, and `BodyId::Root` to
   `BodyId::Definition(component)`, each with its `BodyIds`.
4. **Map each carried violation** to the target model's ids:
   - `Geometry(body, v)`: the new body, and `v` with every vertex, edge, face or curve id
     replaced through that body's maps. Pairs (`DuplicateVertex`, `DuplicateEdge`,
     `EdgesIntersect`, `DuplicateFace`) are written lower id first after mapping, as
     `validate` writes them.
   - `BadTransform(body, instance)`: the new body and the new instance id.
   - `GroupInstances(d, n)`: `GroupInstances(definition_map[d], n)`.
   - A violation with an id that has no mapping (the clipboard leaves out a stray vertex of
     the source's root, and a curve `carry` couldn't carry) is dropped from the allowed
     list: it can't appear in the result anyway.
5. **Added** = the violations of `model.validate()` after the copy that are neither in
   `before` (the model's own faults, as today) nor in the mapped carried list. If any are
   left: roll back and `Err(CopyError::Invalid(added))`, as today.

The copy is exact (raw inserts into new bodies), so in practice nothing is added; the check
stays as the safety net that keeps a broken copy out of the model. Don't replace it with
"allow anything in the new definitions" or with counts: the tests check that each carried
fault names the copies of the source's own entities.

Write a short doc comment on `import_component` saying that the source's own violations are
carried and only added ones refuse, and update the `CopyError::Invalid` comment to match.

## The message: `model_source::import_failure_message(name, error)`

`name` is the definition name the editor got (the file name without its extension).

- Trim `name`; a blank name reads `Component`.
- `CopyError::Empty` -> `Nothing to import`.
- `CopyError::Invalid(added)` with `n = added.len()` ->
  `Can't import {name}: it would add 1 problem to the model` for one,
  `Can't import {name}: it would add {n} problems to the model` otherwise.
- Every other error (`BadMatrix`, `Missing`, `Model`) ->
  `Can't import {name}: its geometry couldn't be copied`.
- Always one line: replace each `\r` and `\n` left in the message with a space (after
  trimming, so `"two\nlines\r"` reads `two lines`).

The editor keeps the message as its own and returns it; the app shows it once in the status
bar (the app change is on main already).

## Done when

- Both test files pass, and every other test in `drydock-kernel` and `drydock` still passes
  (`import_component.rs`, `curves_carry.rs`, `components_import_ui.rs`,
  `model_source_wire.rs` in particular).
- `cargo fmt` and clippy `-D warnings` are clean.

## Where to look

- `drydock_kernel::copy::import_component`, `copy_body_into`, `extract`, `Clipboard::model`,
  `definition_order`, `rollback`, `CopyError` (all in `copy.rs`).
- `Model::validate`, `ModelViolation`, `Model::add_instance`, `InstanceId`, `BodyId`,
  `DefinitionId` (`model.rs`).
- `Violation`, `VertexId`, `EdgeId`, `FaceId`, `CurveId`, `Context::insert_vertex`,
  `Context::insert_edge`, `Context::insert_face` (`topology.rs`).
- `drydock_kernel::curve::carry` (its return value).
- `drydock::model_source::import_failure_message`, `import_component_from`,
  `fetch_component`; `Editor::import_component` (read only).
