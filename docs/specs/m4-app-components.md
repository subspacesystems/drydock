# m4-app-components: make component, make group, explode in the editor

Two `Editor` methods (`app/crates/modeler/src/editor.rs`, on main as `unimplemented!()`)
wrap the kernel calls `modeler_kernel::component::make_component` (with
`unique_definition_name`) and `modeler_kernel::explode::explode`; read
`docs/specs/m4-make-component.md` and `docs/specs/m4-explode.md`. Only `editor.rs` changes.
Tests: `app/crates/modeler/tests/editor_components.rs`. Menus and the name prompt come in a
later task.

Both act on the **open body** (`path`, see `docs/specs/m4-app-edit-context.md`), and
both start like `erase_selection`: any shape in progress, typed text, push/pull and hint
are dropped.

## `make_component(model, name, group)`

- Empty selection: message `Select something to make a component of` (group: `... to
  group`), returns false, nothing changes.
- Otherwise call the kernel with the open body, the selection, `DefinitionInfo { name,
  group }` (name passed through `unique_definition_name`, blank means the default) and label
  `Make Component` or `Make Group`.
- Success: the selection becomes just the new instance, the message is cleared, returns
  true.
- Kernel error: message `Can't make that a component` (group: `Can't group that`), model,
  undo stack and selection unchanged, returns false.

## `explode_selection(model)`

- The selection must be exactly one `Entity::Instance` of the open body. Otherwise message
  `Select one group or component to explode`, false, nothing changes.
- Call the kernel `explode` with the open body, that instance and label `Explode`.
- Success: the selection becomes the returned entities, message cleared, true.
- Kernel error: message `Can't explode that`, nothing changes, false.

## Not in scope

Menu items, shortcuts, name prompt, drawing changes.
