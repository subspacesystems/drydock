# m6-components-panel: the Components panel widget

Milestone M6 (UI). Crate `modeler`: `src/components_panel.rs` (skeleton on main).

## Goal
A panel listing the model's component definitions with small drawn pictures, a search box and a list / tiles switch. Clicking an entry says "place this one". Placing itself (the instance following the cursor) is a later task; this one is the widget and its two helper functions.

## Already on main
The skeleton: `ComponentsPanel { search, tiles }`, `ComponentAction::Place`, `listed`, `thumbnail_segments`, `components_panel`, `LIST_THUMB`, `TILE_THUMB`. Also `icons::icon_button`, `Icon::{List, Tiles}`, and `materials_panel` as the pattern to follow.

## Rules
- `listed(model, search)`: definitions that are components (`DefinitionInfo.group == false`; groups never appear) and whose name contains `search`, ignoring case (a blank search matches all). Order: name ignoring case, ties by ascending id.
- `thumbnail_segments(model, definition)`: one segment per edge of the definition's own geometry (instances inside it are not drawn), in edge-id order, each as the edge's two end points in order. Project a point (x, y, z) to `px = (x - y) * cos 30°`, `py = -(z + (x + y) * sin 30°)` (screen y points down). Then shift and scale all points by one factor so the larger of the drawing's two extents becomes exactly 1 and starts at 0, and the other axis is centred in `[0, 1]`. A definition with no edges (or an unknown id) gives no segments.
- `components_panel(ui, panel, model)`, top to bottom:
  1. A heading `Components`.
  2. A single-line text box with hint `Search`, bound to `panel.search`.
  3. One row of two icon buttons, like the Materials panel's: `List` (tooltip `Show as a list`, selected when `!panel.tiles`) and `Tiles` (tooltip `Show as tiles`, selected when `panel.tiles`). Clicking sets `panel.tiles`.
  4. The entries from `listed(model, &panel.search)`:
     - List: one row per entry, the whole row one button whose accessible label is the definition name; the row holds a `LIST_THUMB` square picture (the thumbnail segments drawn in the theme's text color, 1 pt lines) and the name to its right. Rows are stacked top to bottom.
     - Tiles: tiles of a `TILE_THUMB` picture with the name under it, each tile one button labelled with the definition name, wrapped left to right and then down as the width allows.
     - If the model has no components at all: the text `No components`. If it has some but the search matches none: `No matches`.
  5. Returns `Place(id)` when an entry is clicked.
- The panel must fit the width it is given (no horizontal scroll bar; names are truncated, not wrapped, in the list).

## Acceptance
- `cargo test -p modeler --test components_panel`.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
