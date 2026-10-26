# m6-entity-info: the Entity Info panel and renaming

Milestone M6 (UI). Crate `modeler`: `src/entity_info.rs` and two methods in `src/editor.rs`.

## Goal
A panel that tells the user what is selected in the open body: what kind of thing it is, how long or how large, its size, its materials, and for a single group or component its names, with fields to rename them. Renames go through the editor as one undo step. Hosting the panel in a dock column comes in a later task (`m6-dock-ui`); here it is a function that draws into a `Ui`, like the Materials panel.

Limit (ADR-005): the panel always describes the whole selection and lives in its own panel or window. Don't add attribute pop-ups at the click position in the viewport, or filter the fields by where the user clicked.

## Already on main
`src/entity_info.rs`: the types (`InfoKind`, `Shared`, `InstanceInfo`, `EntityInfo`, `InfoAction`, `EntityInfoPanel`) and stubs for `describe`, `title`, `format_area`, `material_label`, `entity_info_panel`. `src/editor.rs`: stubs `Editor::rename_definition` and `Editor::rename_instance`.

## describe(model, body, selection)
- Entities `body` doesn't have are left out (a missing body has none). `edges`, `faces`, `instances` count the rest.
- `kind`: nothing left → `Nothing`; only edges → `Edges`; only faces → `Faces`; only instances, all of group definitions → `Groups`; only instances, none of them groups → `Components`; anything else (two kinds, or groups with components) → `Mixed`.
- `length`: only for `Edges`; the sum of the edges' lengths.
- `area`: only for `Faces`; the sum of each face's area (outer loop's area minus its holes').
- `front` / `back`: only for `Faces`; `Same(m)` when every selected face has front (back) material `m` (the body's `face_materials`; `None` = default), otherwise `Varies`.
- `size`: `max - min` of `modeler_kernel::tools::scale::bounds(model, body, kept)` (the kept entities), `None` when that is `None`.
- `instance`: only when exactly one entity is kept and it is an instance: its id, definition id, the definition's `group` flag and name, the instance's `name` and `material`, and `copies` = `model.instances_of(definition).len()`.

## Text
- `title`: `Nothing` → `No Selection`; `Edges` → `Edge` for one, else `<n> Edges` (n = `edges`); likewise `Face`/`Faces` (`faces`), `Group`/`Groups` and `Component`/`Components` (`instances`); `Mixed` → `<edges + faces + instances> Entities`.
- `format_area(m2, unit)`: the area in the square of `unit`'s length (feet for `Foot`), rounded to 2 decimals with trailing zeros and a bare point dropped, then the suffix `mm²`, `cm²`, `m²`, `in²` or `ft²`; no space. Not finite gives `0` and the suffix.
- `material_label`: `Same(None)` → `Default`; `Varies` → `Varies`; `Same(Some(id))` → the material's name, `?` when the model doesn't have it.

## The panel: entity_info_panel(ui, panel, model, body, selection, unit)
No heading of its own (the dock shows "Entity Info"). From top to bottom, each value its own label (so tests find it by text):
- The `title` line.
- `Edges`: a row `Length` with `units::format_length(length, unit, 2)`.
- `Faces`: rows `Area` (`format_area`), `Front` and `Back` (`material_label`).
- One component: a text field labelled `Definition` (the definition's name), a text field labelled `Instance` (the instance's name), rows `Material` (`material_label(Same(instance material))`) and `In model` (`copies` as a whole number).
- One group: a text field labelled `Name` (the definition's name) instead of the two fields, and the `Material` row. No `In model`.
- Whenever `size` is `Some`: rows `Width`, `Depth`, `Height` with x, y, z through `format_length(_, unit, 2)`.
- A text field's accessible label is its row label (use `labelled_by` with the row label's id), so `get_by_label("Definition")` finds the field itself.

The fields' text lives in `panel.definition_name` / `panel.instance_name`. `panel.shown` is `Some((instance, definition name, instance name))` of what they were last filled from. Each frame with one instance described: when `shown` differs from the current values (another instance selected, or a name changed in the model by undo or anything else), refill both fields and set `shown`. With no single instance: `shown = None`, both fields empty, no text fields drawn.

A field commits when it loses focus (Enter, or clicking elsewhere):
- Definition / Name field: its text trimmed; blank, or equal to the definition's current name → no action and the field goes back to the current name; else return `RenameDefinition(definition, trimmed)`.
- Instance field: trimmed (blank allowed); equal to the current name → nothing; else `RenameInstance(instance, trimmed)`.
- Escape in a field: no action; the field goes back to the current name.

At most one action per frame, and a rename is returned once: on the frame it is returned, set `panel.shown = None`, so the next frame refills the fields from the model (by then the app has applied it; egui may run a frame's UI twice, and the field reports losing focus in both). The app applies it with the editor (in `m6-dock-ui`).

## Editor
- `rename_definition(model, id, name) -> bool`: `name` trimmed. Blank, an unknown `id`, or the definition's current name → `false`, nothing changes, no undo step. When another definition already has the name, append ` <n>` to the trimmed name with the smallest `n >= 2` that no other definition has (the definition's own name doesn't count as taken; `Table 2` asked for while `Table 2` is taken gives `Table 2 2`). When that lands on its current name: `false`, no change. The `group` flag stays. One undo step labelled `Rename`. `true` when the name changed.
- `rename_instance(model, id, name) -> bool`: instance `id` in the open body (the editor's edit path); `name` trimmed, blank allowed. Not in the open body, or the same name → `false`, no undo step. Only `name` changes; one undo step `Rename`.
- Neither touches the selection or the current tool.

## Acceptance
- `cargo test -p modeler --test entity_info`: describe for nothing, edges, faces with a hole, face materials, one component (names, material, copies), one group, several components, mixed, missing entities and bodies, inside a definition; titles; areas in every unit; material labels; both renames (trim, blank, same, unknown, taken names, group flag, undo); the panel for nothing, edges, a face, a component, a group; renaming by Enter in each field; unchanged and blank names; Escape; fields following the selection and the model; a property test of counts and kinds.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
