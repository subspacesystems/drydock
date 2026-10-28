# m6-eyedropper-2: eyedropper that keeps the tool, toggles, and shows itself

Milestone M6 (UI), feedback round 3. Crate `modeler`: `src/editor.rs`, `src/materials_panel.rs`, `src/icons.rs`, `src/app.rs`.

## Goal
The eyedropper from `m6-eyedropper` confused the user in four ways. Fix all four:
1. After a sample it switched to the Paint Bucket, so the next click (often the second half of a double-click) painted the face the user meant to sample. It must keep the tool that was active.
2. Its icon reads as a magnifying glass. Redraw it as a pipette.
3. Nothing showed it was armed. The button must stay pushed in while armed (click it again to cancel), and over the viewport the mouse cursor must become the pipette.
4. The current material was hard to spot in the tile view and could be scrolled out of sight. The whole current tile gets a selection box, and the panel scrolls to the current material whenever it changes.

## Already on main (skeleton)
- `MaterialAction::CancelEyedropper`, and `app.rs` calling `Editor::disarm_eyedropper()` for it.
- `MaterialsPanel::eyedropper_armed` (pub field, false by default).
- `Editor::disarm_eyedropper()` and `icons::eyedropper_cursor_rect(pointer)` with `unimplemented!()` bodies; `icons::CURSOR_SIZE` = 24.

## Rules
- **Editor.** A sampling click (armed, on a face or instance) makes the sampled material current, clears the message and disarms, as before, but **the tool stays as it was**. So in the Select tool the next click selects, and in the Paint Bucket it paints with the sample. A double-click while armed: the first click samples, the second acts in the unchanged tool (in Select it can never paint). `disarm_eyedropper()` clears the flag and does nothing else (a no-op when not armed). The other rules from `docs/specs/m6-eyedropper.md` stand: edges and empty space keep waiting, Esc and `set_tool` disarm, no undo step, no selection change.
- **Button.** Draw the eyedropper button with `selected = panel.eyedropper_armed`. A click returns `MaterialAction::CancelEyedropper` while armed and `MaterialAction::Eyedropper` otherwise. The app sets `panel.eyedropper_armed` from `editor.eyedropper_armed()` before drawing the panel each frame, so the button pops back out after a sample, Esc or a tool change.
- **Icon.** Redraw `Icon::Eyedropper` as a pipette lying on the diagonal: a **solid** rounded bulb at the top right, a short collar across the diagonal below it, then a narrow tube (two parallel strokes, or one thick stroke) running to a pointed tip at the bottom left. No hollow ring. It must still pass the icon tests (centred, ink minimum, distinct from every other icon).
- **Cursor.** `eyedropper_cursor_rect(pointer)` returns the `CURSOR_SIZE` square in which `icons::shapes(Icon::Eyedropper, rect, _)` puts the pipette's tip (the ink furthest towards the bottom left) within 2.5 pt of `pointer`. In the app, while the editor's eyedropper is armed and the pointer is over the viewport: set `egui::CursorIcon::None` and paint the icon in that rect on a foreground layer (theme text color, with a one-pt contrasting outline or shadow so it shows on dark and light faces). Elsewhere the normal cursor.
- **Tiles.** In the tile view each tile is **one** widget: allocate the whole tile (64 pt wide, swatch 40 pt plus the name line) with `Sense::click()`, give it `WidgetInfo::selected(WidgetType::Button, enabled, selected, name)`, and paint, in order: when selected, a filled box behind the whole tile in `visuals.selection.bg_fill` with a 1 pt `visuals.selection.stroke` border (hover: `weak_bg_fill`); the swatch; the name centred under it, truncated with an ellipsis to the tile width, in `selection.stroke.color` when selected. Tooltip and click behaviour as before (a click anywhere on the tile chooses it). The list view keeps its rows.
- **Scroll to current.** The panel remembers which material was current when it last drew (a private field, `Option<Option<MaterialId>>`, `None` before the first draw). When `current` differs from it (including the first draw), the current entry, list row or tile, calls `scroll_to_me(None)` once. Otherwise never scroll on its own: the user's wheel scrolling must stick. If the current material is hidden by the search text, nothing scrolls.

## Notes
- A throwaway implementation that passes these tests touched about 80 lines (editor 3, icons 11, panel 66); the tile painted its name with a truncating `LayoutJob`.
- The wheel scroll in egui is animated: `scroll_to_me` likewise needs a frame or two.

## Acceptance
- `cargo test -p modeler --test eyedropper` (editor keeps the tool, double-click, `disarm_eyedropper`, button pushed in and cancel) and `--test eyedropper_2` (icon shape, cursor tip, tile box, scroll to current, no scroll on its own).
- `--test materials_swatch`: `a_tile_shows_the_image_above_its_name` now finds the tile as one widget and allows the swatch an inset (52–64 by 32–40 pt) so the selection box shows around it.
- Other existing tests unchanged (notably `materials_layout`, `materials_icons`, `icons`, `icons_2`), `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
