# m6-toolbar-customize-3: Customize window without per-tool buttons; rename a toolbar

Milestone M6 (UI), feedback round 3. Crate `modeler`: `src/toolbars.rs`.

## Goal
Dragging between the two lists now does everything the old per-tool buttons did, so they go. The user can also rename a toolbar, not only create and delete one.

## Already on main
`customize_panel`, `CustomizeState`, `ToolbarLayout::{add_toolbar, remove_toolbar, reset}`, the two-list drag (`m6-toolbar-customize-2`). Skeletons: `ToolbarLayout::rename_toolbar` and the `CustomizeState::rename` field.

## `ToolbarLayout::rename_toolbar(index, name) -> bool`
- Out of range: false, nothing changes.
- Clean the name as `add_toolbar` does: every `\r` and `\n` becomes a space, then trim. Empty after cleaning: false.
- When another toolbar (not `index` itself) already has the name, append ` 2`, ` 3`, ... (first free), as `add_toolbar` does.
- When the result equals the toolbar's current name: false. Otherwise set it and return true. Nothing else about any toolbar changes.

## Customize window
- Remove the `Remove X`, `Add X`, `Move X earlier` and `Move X later` buttons. Each row keeps its drag handle (`Drag X`) and the tool's name; the drag behaviour is unchanged.
- The new-toolbar text box label becomes `New toolbar name:` (with the colon).
- Add a row for the selected toolbar: a label `Toolbar name:`, a single-line text box labelled by it (`labelled_by`), and a button `Rename toolbar`.
- `state.rename` is `Some((index, text))`: the text box edits `text`. Whenever it is `None` or its index differs from `state.selected`, fill it with `(selected, that toolbar's name)` before drawing the box. With no toolbars at all, leave it `None` and skip the row.
- Clicking `Rename toolbar`, or pressing Enter in the text box (the box loses focus with Enter pressed), calls `rename_toolbar(selected, text)`; the panel returns true when that did. Typing alone renames nothing.
- After a rename attempt (changed or not), and after Create toolbar, Delete toolbar and Reset Toolbars, set `state.rename` to `(selected, the selected toolbar's current name)` in the same frame, so the box shows the stored name (e.g. `Camera 2`, or the old name back after an empty one).
- Everything else (picker, create, delete, reset, return value) as before.

## Acceptance
- `cargo test -p modeler --test toolbar_customize_rename --test toolbar_customize --test toolbar_customize_lists`.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
