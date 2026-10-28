# m6-toolbar-side-width: side toolbars as narrow as the toolbar

Milestone M6 (UI), feedback round 3. Crate `modeler`: `src/toolbars.rs` (`edge_panel`), `src/app.rs` (`draw_toolbars`).

## Goal
A toolbar dragged to the left or right edge sits in a column about 200 pt wide with a resize handle, and the handle can't make it as narrow as the buttons. The column should be exactly as wide as a toolbar and have no resize handle.

## Already on main (skeleton)
`toolbars::SIDE_MARGIN` (3 pt) and `toolbars::edge_panel(edge, style) -> egui::Panel` with an `unimplemented!()` body.

## Rules
- `edge_panel(Edge::Top | Edge::Bottom, _)`: the same panels `draw_toolbars` uses today (`egui::Panel::top("toolbars-top")`, `egui::Panel::bottom("toolbars-bottom")`), behaviour unchanged.
- `edge_panel(Edge::Left | Edge::Right, style)`: `Panel::left("toolbars-left")` / `Panel::right("toolbars-right")`, not resizable, with an exact width of `icons::BUTTON_SIZE + 2 * SIDE_MARGIN`. The panel's frame keeps the usual panel fill (`Frame::side_top_panel(style)`), but its left and right inner margins are `SIDE_MARGIN`, so the buttons fit and sit centred. A width saved from an earlier run must not matter (an exact width does that).
- `draw_toolbars` in `app.rs` gets its four panels from `edge_panel` instead of building them itself. Nothing else in it changes (strip drawing, rects, drag outline).

## Acceptance
- `cargo test -p modeler --test toolbar_side` (left and right column width and position, buttons inside and centred, two toolbars in one column, dragging the border does not resize, top and bottom unchanged).
- All existing tests unchanged, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
