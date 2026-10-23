# m3-erase: erase the selection (kernel)

Implement `erase` in `app/crates/modeler-kernel/src/erase.rs` (skeleton on main). Change only
that file. Acceptance tests: `app/crates/modeler-kernel/tests/erase.rs` and
`tests/erase_props.rs` (they use the shared helpers in `tests/transform_common/`).

```rust
pub fn erase(model: &mut Model, body: BodyId, selection: &[Entity],
             undo: &mut UndoStack, label: &str) -> Result<(), EraseError>
```

## Checks, in this order (nothing changes on error)

1. `selection` is empty: `EraseError::Empty`.
2. `body` doesn't exist: `EraseError::Model(ModelError::MissingBody(body))`.
3. An edge, face or instance in `selection` is not in the body: `EraseError::Missing(entity)`
   for the first such entity in selection order.

Duplicates in `selection` are allowed and count once.

## What is erased, in this order

All steps go into one `ModelEdit`; on success push it once onto `undo` with `label`.

1. **Faces.** Each selected face (ascending id) is removed with `Context::delete_face`. Its
   edges and vertices stay. Face material entries are left alone (the model keeps entries
   of removed faces so undo brings materials back; see the `Body` doc comment).
2. **Edges.** Each selected edge (ascending id) that is still present is erased with
   `Context::erase_edge` (sticky.rs), which already does the right thing:
   - a face whose outer loop uses the edge goes;
   - a hole bordered by the edge closes (the face around it keeps its id, minus that inner
     loop);
   - when exactly two faces use the edge and they are coplanar and face the same way, they
     merge into one face instead;
   - end vertices left without edges go.
   Faces in step 1 go first, so an edge whose faces were all selected is erased plainly
   (no merge).
3. **Instances.** Each selected instance (ascending id) is removed with
   `Model::remove_instance`. Its definition stays.

Run the geometry steps through `Model::edit_geometry` (one call or several, either way).

If any step fails, undo what this call did so far (apply the inverse of the records logged
so far, newest first), leave `undo` untouched and return the error: `Sticky(e)` for
`erase_edge`, `Model(e)` for model records. The tests can't easily reach this path, but keep
it correct.

## Not in scope

- Guides (not selectable yet), definitions without instances (purge comes later).
- Clearing the editor's selection: the app task (`m3-app-erase`) does that.
