# m6-components-import: turn a model into a component definition

Kernel half of "import a file as a component". Fill in `import_component` in
`app/crates/modeler-kernel/src/copy.rs` (the stub is there), reusing the machinery `insert`
already has for copying geometry, face materials, instances and the definitions and
materials they reach from one model into another. Tests:
`app/crates/modeler-kernel/tests/import_component.rs`.

## Behaviour

`import_component(model, source, name, undo, label)` takes everything in `source`'s root
(edges, faces, face materials, instances, plus every definition and material those reach)
and puts it inside a new, non-group definition of `model`.

- The new definition's name is `name` made unique with `unique_definition_name` (trimmed;
  blank becomes "Component"; "Table 2", "Table 3" ... when taken).
- Points keep their coordinates (identity transform). No instance is placed; the root of
  `model` is unchanged.
- Definitions and materials from `source` are added to `model` the same way paste adds
  them (existing equal materials are reused, a clashing material name is made unique;
  nested definitions are copied as new definitions).
- `Err(CopyError::Empty)` when the source root has no edge, face or instance.
- Any other failure (model refuses a record, validation violations that were not there
  before) is returned as the matching `CopyError`.
- On every error the model and the undo stack are exactly as before.
- On success it is one undo step labelled `label`; undo removes the definition and
  everything added with it, redo brings it back.
- `source` is never changed.

## Out of scope

The Import button in the Components panel, the file dialog and reading the file
(`modeler_dae::import_file`), and placing the result (`m6-components-place`): a later
task wires them in the app.
