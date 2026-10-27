# m6-panel-stack: docked panels that stack, scroll and resize on their own

Milestone M6 (UI), feedback round 1. Crate `modeler`: `src/dock.rs`, `src/app.rs`, panel widgets.

## Goal
Docked panels share a column but behave independently: each scrolls on its own, the border between two can be dragged, a title is shown once, and the View menu reveals a panel rather than hiding it. The tested parts are pure functions in `dock.rs` plus the removed headings; the app drawing is checked by hand in the real window.

## Already on main
Skeletons in `src/dock.rs`: `MIN_BODY`, `StackRow`, `stack_heights`, `drag_splitter`, `DockLayout::reveal`. Replace the `unimplemented!` bodies.

## Rules for the tested items
- `DockLayout::reveal(id)`: the panel becomes open and not collapsed. It never closes anything and keeps the panel's place (side, slot, floating rect).
- `stack_heights(available, rows)`: one height per row, header included.
  - A collapsed row is `HEADER_HEIGHT`.
  - The rest share what is left (`available` minus the collapsed rows) in proportion to `weight`; a weight that is not positive counts as 1.
  - No expanded row gets less than `HEADER_HEIGHT + MIN_BODY`. A row whose share would be smaller is pinned at that minimum and the others share what remains (repeat until none is pinned). If even the minimums do not fit, every expanded row gets the minimum (the total exceeds `available`).
  - Empty input gives empty output.
- `drag_splitter(rows, heights, index, delta)`: the border under row `index` moves `delta` points down (negative is up). Row `index` grows by `delta` and row `index + 1` shrinks by the same, each kept at or above `HEADER_HEIGHT + MIN_BODY` (the move is clamped). Afterwards every expanded row's `weight` is its height from `heights` (with the two changed), so `stack_heights` with the same `available` reproduces those heights. Nothing happens when `index + 1` is out of range or either row is collapsed.
- Headings: the Materials, Components and Shadows widgets no longer draw their own title (the panel header already shows it). Existing tests were updated to say so. Texture Position keeps its heading until `m6-texture-panel`.

## App wiring (checked by hand, not by tests)
- `App` keeps one `StackRow` list per column (weights are not saved to the layout file).
- Each docked panel is a header plus, unless collapsed, its own `ScrollArea` (distinct id per panel) of the height `stack_heights` gives. Scrolling one panel's contents never moves another. The column itself does not scroll, except when the minimums do not fit.
- Between two expanded panels, a thin draggable border (resize cursor) feeding `drag_splitter`.
- View menu panel items call `reveal` (and bring a floating panel to the front), not `toggle`: they never hide. Only the header's close button closes. The menu item still shows a check when the panel is open. Closing Shadows still clears the shadow preview.
- Header buttons: draw the collapse chevron and the close cross with the painter (lines), not with the text glyphs `v`, `>` and `x`; same size, accessible labels and events (the `dock_ui` tests must pass unchanged).

## Acceptance
- `cargo test -p modeler --test dock_stack`, plus `dock_ui`, `components_panel`, `materials_layout`, `materials_panel`, `shadows_panel` unchanged by you.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
