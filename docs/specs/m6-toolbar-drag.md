# m6-toolbar-drag: where a dragged toolbar lands

Milestone M6 (UI), feedback round. Crate `modeler`: `src/toolbars.rs`, `src/app.rs`.

## Goal
Dragging a toolbar by its grip shows an outline of where it will land, and dropping it between two docked toolbars puts it there (today it always goes to the end of the edge).

## Already on main
`toolbar_strip`, `ToolbarEvent::Dropped`, `drop_target`, `ToolbarLayout::dock`. Skeletons in `src/toolbars.rs`: `insertion_index`, `drop_outline`.

## Rules for the tested items
- `insertion_index(edge, rects, pos)`: `rects` are the screen rectangles of the toolbars docked along `edge`, in docked order. Along the edge (x for Top and Bottom, y for Left and Right) count the rectangles whose centre is before `pos`'s coordinate (strictly less). The result is in `0..=rects.len()`. No rectangles: 0.
- `drop_outline(edge, rects, index, window, size)`: a rectangle of exactly `size`.
  - Along the edge its start is `rects[index]`'s start, or the end of the last rectangle when `index == rects.len()`.
  - Across the edge its start is `rects[0]`'s start (top for Top/Bottom rows, left for Left/Right columns).
  - With no rectangles: along the edge it starts at `window.left()` (Top, Bottom) or `window.top()` (Left, Right); across the edge it sits against the edge: Top at `window.top()`, Bottom ending at `window.bottom()`, Left at `window.left()`, Right ending at `window.right()`.

## App wiring (checked by hand, not by tests)
- While a grip is being dragged, each frame compute `drop_target(pointer, window)`. With a target, draw `drop_outline` (from the recorded rectangles of that edge's strips, `size` the dragged toolbar's own size) as a stroked rectangle in the theme's selection color on a foreground layer; with none, draw an outline of the dragged toolbar's size with its top-left at the pointer (the float position).
- On `Dropped`, dock at `insertion_index(edge, rects, pos)` instead of the end. Record each docked strip's rectangle while drawing (`Response::rect` of the strip).
- Dragging a toolbar within its own edge: leave its own rectangle out of `rects` and compute the index from the rest, so it can move left or right.

## Acceptance
- `cargo test -p modeler --test toolbar_drag`.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
