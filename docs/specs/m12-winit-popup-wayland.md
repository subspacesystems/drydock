# m12-winit-popup-wayland: pop-up windows as xdg_popup on Wayland

Milestone M12. `m12-winit-popup` added the pop-up window kind for X11 and Windows;
on Wayland it logs a warning and makes an undecorated toplevel. This task makes it a real
`xdg_popup` so a menu can reach past the main window's edge. Needs `m12-winit-popup` merged.

## Limit of the tests (read first)

CI has no Wayland compositor. The acceptance tests cover only the placement math, as two pure
functions. The `xdg_popup` plumbing (create, grab, configure, done) cannot be run here: write it
carefully against sctk's API, make it compile, and keep every code path that talks to the
compositor small. Do not try to start a compositor.

## What is already on main

`app/vendor/winit/src/platform/wayland.rs` (public, Linux with the `wayland` feature) ends with
`PopupRect`, `PopupAdjustment`, `PopupPositioner` and two functions with `unimplemented!()` bodies:

- `popup_positioner(content_offset, requested, size) -> PopupPositioner`: anchor rect is the 1 x 1
  rect at `content_offset + requested`; size as given; offset (0, 0); all four adjustment flags
  true. Anchor (top-left) and gravity (bottom-right) are fixed, so they are not fields.
- `resolve_popup_position(positioner, parent_origin, bounds) -> (i32, i32)`: a model of the
  `xdg_positioner` rules, used only by tests. Per axis, independently:
  1. unconstrained position = `parent_origin + anchor_rect origin + offset` (the pop-up's top-left
     sits on the anchor point);
  2. if it lies outside `bounds` on that axis and the flip flag is set: flipped position = anchor
     rect's far edge minus the pop-up size (`parent_origin + anchor_rect.x + anchor_rect.width -
     width`, plus the offset), taken only if that one is inside `bounds` on the axis;
  3. if still outside and the slide flag is set: move the minimum distance to be inside; when the
     pop-up is larger than the bounds the top/left edge wins;
  4. with no flag nothing moves.

## What to do

1. Implement the two functions in `platform/wayland.rs` (tests: `popup_positioner_wayland.rs`).
2. **Wayland window** (`src/platform_impl/linux/wayland/window/mod.rs`, `Window::new`, and
   `window/state.rs` / `state.rs` for the plumbing): replace the M12 warn-and-undecorated fallback.
   For `attributes.popup()` with a transient parent that is a window of this loop:
   - build an `xdg_positioner` from `popup_positioner(content_offset, position, size)`. The
     position from `with_position` is, for a Wayland pop-up, **logical pixels from the top-left
     of the parent's content area** (Wayland clients cannot know screen coordinates). The content
     offset is the parent's client-side frame offset (the title bar height under sctk-adwaita;
     (0, 0) with server-side decorations). Size is `attributes.inner_size` (default if none),
     logical;
   - create an `xdg_popup` (sctk `shell::xdg::popup::Popup`) with the parent's `XdgSurface`; the
     popup gets its own `wl_surface` and must be reachable through the same window-id lookup
     winit uses for toplevels, so wgpu can draw on it and the pointer, keyboard and redraw
     events find it;
   - grab it right after creation, before the first commit: `xdg_popup.grab(seat, serial)` with
     the seat and the latest button (or key) serial winit already tracks for the pointer
     (`WinitPointerData::latest_button_serial`). If no seat or serial exists, create the popup
     without a grab and log at `debug`;
   - implement sctk's `PopupHandler` for `WinitState` and `delegate_xdg_popup!`: `configure`
     applies the size the compositor chose and requests a redraw; `done` (dismissed by the
     compositor, a click outside, Escape) sends `WindowEvent::Focused(false)` and then
     `CloseRequested` for that window, so the app closes it the same way as on X11;
   - dropping the `Window` destroys the popup (xdg_popup, xdg_surface, wl_surface) in the right
     order. Several pop-ups may be open at once (a submenu is a popup of a popup): when the
     transient parent is itself a popup, anchor to it the same way.
   - `set_outer_position`/`outer_position` stay unsupported; no decorations, no frame, no title.
3. If the parent is not a window of this loop, or the compositor lacks `xdg_wm_base` popups
   (never in practice), fall back to what `m12-winit-popup` does (warn once, undecorated toplevel).
4. Do not touch X11, Windows or any file outside `src/platform_impl/linux/wayland/`,
   `src/platform/wayland.rs` and the two `PATCHES.md` files. Windows without `popup` must behave
   exactly as before.
5. **Docs**: update the `with_popup` doc comment in `src/window.rs` (Wayland line: `xdg_popup`
   with an input grab; the position is relative to the parent's content area) and the "Pop-up
   windows" group of `app/vendor/winit/PATCHES.md` (one bullet per file and function). The
   `m12-popup-window` helper has to pass a content-relative position on Wayland; say that in the
   PATCHES.md bullet so that task's author sees it.

Mark every change `// Local patch (Drydock, PATCHES.md): ...`; `cargo fmt` does not touch
`app/vendor`, so format by hand in the file's style.

## Acceptance tests

`app/crates/drydock/tests/popup_positioner_wayland.rs` (do not edit): positioner fields
(title bar offset, none, negative request, flags); resolve: room to spare, parent origin and
offset, reaching past the parent, flip_y / flip_x, flip only when it fits, slide alone, flip with
slide off, second monitor, larger than the bounds, axes independent.

Run: `cargo test -p drydock --test popup_positioner_wayland`; the rest of the workspace and the
X11 pop-up test (`popup_window_x11`) must still pass; clippy as usual (`app/vendor` is not
linted).

## Where to look

`show` / `outline`: `Window::new` (wayland/window/mod.rs, the transient-parent and modal code),
`WindowState` and its `frame` field (`DecorationsFrame::location` gives the content offset; wayland/window/state.rs),
`WinitState` and its `WindowHandler` impl and `delegate_*` lines (wayland/state.rs),
`WinitPointerData::latest_button_serial` (wayland/seat/pointer/mod.rs), `Window::drop`.
sctk: `smithay-client-toolkit-0.19.2/src/shell/xdg/popup.rs` (`Popup`, `PopupHandler`,
`delegate_xdg_popup!`) and `shell/xdg/mod.rs` (`XdgPositioner`, `XdgShell`).
