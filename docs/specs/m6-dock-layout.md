# m6-dock-layout: side panels that dock or float

Milestone M6 (UI). Crate `modeler`: `src/dock.rs`.

## Goal
The data behind dockable panels (Entity Info, Materials, Components, Shadows): which are open, which sit in the left or right column and in what order, which are collapsed to their title row, which float as windows (where, how big), and how wide each column is. Also the two small rules the UI uses while a panel is dragged: which column it would drop into and at what position. The layout is kept in a text file between runs. Drawing and dragging come in a later task.

## Already on main
`src/dock.rs`: `PanelId`, `PANELS`, `Side`, `Place`, `Panel`, `DropTarget`, `DockLayout`, the constants, `is_open`, `toggle`, `reset`, `load`, `save`, `title`, `panel_key`, `panels_file` (done), and stubs for the rest.

## Layout rules
- The layout always holds exactly one `Panel` per `PanelId`; `panel(id)` returns it (`panel(id).id == id`).
- `Default`: Entity Info and Materials open, Components and Shadows closed; all four docked `Right` with slots 0, 1, 2, 3 in `PANELS` order; none collapsed; both widths `DEFAULT_WIDTH`.
- Order in a column is by `slot`. After every method, the slots of all panels docked on a side (open and closed) are exactly 0, 1, 2, ... in order.
- `docked(side)`: the open panels docked on `side`, top to bottom. `floating()`: the open floating panels in `PANELS` order. Closed panels are in neither but keep their place.
- `set_open`, `set_collapsed`: only that flag changes (a floating panel may carry the collapsed flag; it matters only when docked).
- `dock(id, side, position)`: opens it and docks it on `side` so that `docked(side)[position] == id` (or last when past the end). Other panels on that side keep their relative order; a closed one stays just where it was (insert before the open panel that was at `position`). Docking on its own side just reorders; the side it left closes its gap.
- `float(id, rect)`: when `rect`'s corner and size are all finite, opens it as `Floating` at `rect.min` with size `rect.size()` raised to at least `MIN_FLOAT_SIZE` (each axis), closing the gap in its old column. Otherwise nothing changes.
- `width(side)` / `set_width(side, w)`: `w` clamped to `MIN_WIDTH..=MAX_WIDTH` (infinities clamp too); NaN is ignored.

## Drop rules
- `drop_side(screen, pointer)`: a side's reach is `EDGE_ZONE`, or its column width when larger, but only while it has an open docked panel (an empty column reaches `EDGE_ZONE`). The pointer is in the left zone when `pointer.x <= screen.left() + reach(Left)`, in the right zone when `pointer.x >= screen.right() - reach(Right)` (positions past the window's edges count). In both zones (a narrow window), the nearer edge wins, the left on a tie. In neither: `None` (float). The pointer's y doesn't matter.
- `drop_position(middles, y)`: the number of `middles` strictly above `y` (`middle < y`).

## File format
```
modeler panels 1
width left 200
width right 300.5
panel entity-info 1 0 right 0
panel materials 1 1 right 1
panel components 0 0 left 0
panel shadows 1 0 float 100 50 260 360
```
- First line exactly `HEADER`; then `width left <w>`, `width right <w>`, then one `panel` line per panel in `PANELS` order: `panel <key> <open> <collapsed> left|right <slot>` or `panel <key> <open> <collapsed> float <x> <y> <w> <h>`. Keys are `panel_key`; flags `1`/`0`; numbers printed with `{}`. `to_text` writes exactly that, ending with a newline. For any layout the methods produce, `parse(to_text())` gives it back equal.
- `parse` accepts `\r\n` and blank lines, and fields separated by any whitespace. It starts from the default layout and applies each line it understands, so panels and widths the file doesn't mention keep their default. Text whose first line isn't `HEADER` gives the default.
- Skipped lines: an unknown side or key, a flag other than 0/1, a slot that isn't a whole number, a number that doesn't parse or isn't finite, the wrong number of fields, anything else. Widths go through `set_width`'s clamp; floating sizes are raised to `MIN_FLOAT_SIZE`. A later line for the same panel replaces an earlier one.
- Afterwards slots are renumbered per side, ordered by slot, ties in `PANELS` order.

## Acceptance
- `cargo test -p modeler --test dock_layout`: default; open/close keep the place; dock between columns and reorder; closed panels keep their place; float with minimum size and non-finite rects; collapse flag; width clamp; drop side zones (empty column, open column, shifted window, narrow window); drop position; the example file both ways; defaults for non-layout text; bad lines; load/save; property tests (random edits keep every rule and survive the file; `parse` of random text never panics).
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
