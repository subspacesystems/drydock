# m4-ui-materials-layout: buttons and search on top, a scrolling list or tiles

Milestone M4, from Tyrel's UI list (item 9).

## Problem

In the Materials panel the New / Edit / Purge buttons sit under the list, so with many
materials they are far down the column. There is no way to find a material by name and
the list is one long column of rows.

## What to build (`src/materials_panel.rs`, `src/app.rs`)

`materials_panel` keeps its signature and its actions. Its layout becomes, top to bottom:

1. The heading `Materials`.
2. The button row `New...`, `Edit...`, `Purge Unused` (same labels, same behaviour, still
   wrapping at the column width).
3. A single-line search box (hint text `Search`, full column width) bound to
   `MaterialsPanel::search` (on main).
4. Two selectable buttons labelled `List` and `Tiles`; the one for the current view is
   drawn selected. Clicking one sets `MaterialsPanel::tiles` (on main; `false` = list).
   Both are on one row, wrapping like the button row.
5. A vertical `egui::ScrollArea` (`auto_shrink([false, false])`) holding, in this order,
   the New / Edit Material form when one is open, then the materials. Everything above the
   scroll area stays put however far the list is scrolled and however many materials
   there are; the panel never asks for more height than `ui` has left.

### Search

`matches_search(name, query)` (stub on main): true when `query.trim()` is empty, or when
`name` contains the trimmed query ignoring case (Unicode lowercase on both is fine).

Only materials whose name matches are listed. The `Default` row follows the same rule
with the name `Default`, so searching `oak` hides it. The search never changes the
current material and never sends an action.

### List and tiles

- List view (`tiles == false`): exactly today's rows (swatch then selectable name,
  truncated, tooltip with the name and texture path).
- Tiles view: each material is a tile 64 pt wide, a swatch of the material's colour
  (64 × 40 pt, drawn the way the row swatch is) with the selectable, truncated name
  button under it, laid out left to right and wrapping to the next line at the column
  width (`ui.horizontal_wrapped` or `ui.with_layout(Layout::left_to_right(..).with_main_wrap(true))`).
  The tile has the same tooltip as the row. Clicking the swatch or the name sends the
  same `MaterialAction::Choose` as clicking the row; the current material's name button
  is drawn selected. `Default` is the first tile.
- A long name must not widen the column in either view (the `m4-fix-materials-width`
  rules still hold).

### App

`src/app.rs`: the right panel no longer wraps `materials_panel` in its own
`ScrollArea` (the panel scrolls its list itself); the Texture Position panel stays
pinned at the bottom.

## Tests

`app/crates/modeler/tests/materials_layout.rs` (spec commit; don't edit). Run with
`cargo test -p modeler --test materials_layout`. `name_field_has_focus_when_the_form_opens` in
`tests/materials_panel.rs` changed (the search box is the first text field now). The other `materials_panel` and
`materials_panel_width` tests must keep passing.
