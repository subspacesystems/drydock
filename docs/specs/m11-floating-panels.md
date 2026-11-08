# m11-floating-panels: floating dock panels are their own windows

Tyrel's request: tool windows should be real windows that belong to the main window.
A dock panel (Materials, Entity Info, Components, Tags, Shadows and the rest) that is not
docked becomes its own OS window, kept above the main window, titled with the panel's name.
It builds on `m11-dialog-window` (the same parent-window patch applies to every window after
the root, and `dialog_window` shows how to run a viewport callback with `FnOnce` content).

## What to build

`crates/drydock/src/floating.rs` has the skeleton; fill in the bodies.

- `floating_viewport_id(id)`: `ViewportId::from_hash_of` of a fixed prefix plus the panel's
  `panel_key`; distinct per panel.
- `screen_position(root_inner, layout_pos)` and `layout_position(root_inner, screen_pos)`:
  add / subtract the root's inner top-left corner. Positions saved in the layout file stay
  relative to the main window's inner corner, so existing saved layouts keep their meaning
  and the embedded fallback needs no conversion.
- `floating_panel_window(ctx, id, rect, content)`:
  - **Real window** (`!ctx.embed_viewports()`): `show_viewport_immediate` with
    `floating_viewport_id`, a `ViewportBuilder` titled `dock::title(id)`, inner size
    `rect.size()`, resizable (minimum `dock::MIN_FLOAT_SIZE`), position
    `screen_position(root inner rect, rect.min)` when the root's `inner_rect` is known (read
    it from the calling viewport's input before opening). Inside, a `CentralPanel` with a
    first row holding a "Dock panel" button (the first widget, at the window's top-left;
    the real-window test clicks at 40, 18 from the window corner), a separator, then
    `content`. `close_requested()` on the child pushes `PanelEvent::Close(id)`. The
    returned `rect` is the child's `inner_rect` converted with `layout_position` (size is
    the inner size); `None` when either rectangle is unknown (Wayland gives no positions:
    then keep the stored position and only follow the size). Builder position and size only
    matter when the window is created; do not re-send them each frame.
  - **Embedded** (`ctx.embed_viewports()`, `egui_kittest`): keep today's `egui::Window`
    from `draw_panels` exactly (id `("floating-panel", id)`, no title bar, not movable,
    resizable, `current_pos(rect.min)`, `default_size`, `min_size`), with `panel_header(ui,
    id, false, false)` as the header (its Close and Dropped events go into `events`), then
    the "Dock panel" row, a separator and `content`. `rect` is the window's response rect.
  - `FloatingOutcome::dock` is true on the frame "Dock panel" is clicked. `content` is
    `FnOnce`; keep it in an `Option` and `take()` it inside the `FnMut` viewport callback.
- `raise_floating_panel(ctx, id)`: real: `ctx.send_viewport_cmd_to(floating_viewport_id(id),
  ViewportCommand::Focus)`; embedded: `ctx.move_to_top` of the `("floating-panel", id)`
  window layer (what `reveal_panel` does now). Harmless when the panel isn't shown.
- Wire it into `app.rs`:
  - `draw_panels`: the loop over `layout.floating()` calls `floating_panel_window` with
    `Rect::from_min_size(pos2(x, y), vec2(w, h))` and `|ui| actions.extend(self.panel_body(ui, id))`.
    Its `events` go on the existing `events` list (Close hides the panel exactly as the
    header close button does today, including dropping the Shadows preview). When `dock`
    is set: `self.dock.dock(id, Side::Right, usize::MAX)` (end of the right column), set
    `changed`. When the returned `rect` differs from the stored one by more than 0.5, call
    `self.dock.float(id, rect)` and save, as now.
  - `reveal_panel` calls `raise_floating_panel`.
  - Dropping a panel's header onto an edge of the main window still docks it (embedded and
    for panels dragged out of a dock column). **Dragging a floating OS window back onto the
    main window to dock it can't work across windows in egui**, so the way back is the
    "Dock panel" button. If `docs/guide` has a page on panels, add one sentence saying so.
  - Dragging a docked panel's title out of the dock still floats it (`Dropped` with the
    pointer outside the edge zones); the new window then opens at that spot.
- Floating toolbars are unchanged and stay inline.
- Only egui APIs: no new dependency.

## Acceptance tests

`tests/floating_panels.rs` (do not edit):
- `every_panel_has_its_own_viewport_id`, `layout_and_screen_positions_*`: the pure functions.
- Embedded: header, "Dock panel" row and content in that order; the rectangle reported in
  layout coordinates; Dock and Close reported once clicked; `raise_floating_panel` is safe.
- Real window (Xvfb, xdotool, xprop, `DRYDOCK_GUI_TESTS=1`; skips itself otherwise): writes
  a layout file with Materials floating in a temp `DRYDOCK_CONFIG_DIR`, starts the app, finds
  a second X window titled exactly "Materials" whose `WM_TRANSIENT_FOR` is the main window,
  moves it with `xdotool windowmove` and waits for the saved layout to follow (relative to
  the main window's corner), clicks the Dock panel button and checks the window is gone, the
  main window stays and the layout has Materials docked on the right.

Existing dock tests (`dock_ui.rs`, `dock_layout.rs`, `materials_panel*.rs`, ...) must pass
unchanged. Run `harness/bin/run-tests -p drydock` at the end with Xvfb and
`DRYDOCK_GUI_TESTS=1`.

## Where to look

Use `show <name>`:
- `floating_panel_window`, `raise_floating_panel`, `FloatingOutcome` in `floating.rs`.
- `dialog_window` and `DialogWindow` in `dialog.rs` (viewport pattern, `Option::take` of the
  content); `App::draw_panels`, `App::reveal_panel`, `App::panel_body`, `App::save_panels` in
  `app.rs`.
- `panel_header`, `PanelEvent`, `title`, `panel_key`, `MIN_FLOAT_SIZE`, `DockLayout::dock`,
  `DockLayout::float`, `DockLayout::floating`, `DockLayout::load`/`save`, `panels_file` in
  `dock.rs`; `config_dir_on` in `winpaths.rs` (`DRYDOCK_CONFIG_DIR`).
- egui: `Context::show_viewport_immediate`, `Context::embed_viewports`, `ViewportBuilder`,
  `ViewportInfo::inner_rect`, `Context::send_viewport_cmd_to`, `ViewportCommand::Focus`.
- Test helpers: the real-window helpers (`windows`, `wait_for`, `Kill`) in
  `tests/floating_panels.rs` and `tests/dialog_window.rs`.
