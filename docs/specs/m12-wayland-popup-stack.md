# m12-wayland-popup-stack: Wayland pop-ups keep GNOME's stack rule

## Problem

On GNOME (mutter) Drydock dies with `wl_display.error(xdg_wm_base, 2, "parent not top most
surface")` when one menu is open and another pop-up is created: hovering from File to Edit, or
opening View > Toolbars. The xdg-shell rule: a pop-up that takes a grab must have the topmost
pop-up as its parent. Today every pop-up is created with the root window as parent. Also, a
pop-up is created at 120x40 and later grows with no `xdg_popup.reposition`, so the compositor
keeps it on screen using 40 px, and a tall menu near the bottom runs off the screen.

This task is the winit and egui-winit side. The app side (a submenu naming its menu, and a menu
switch destroying the old pop-up before creating the new one) is `m12-wayland-popup-switch`.
X11 and Windows behavior must not change.

## What to build

All pure pieces are public in `winit::platform::wayland` (`app/vendor/winit/src/platform/wayland.rs`)
or `egui_winit`; the skeletons are on main with `unimplemented!("m12-wayland-popup-stack")` and the
acceptance tests are `app/crates/drydock/tests/popup_stack_wayland.rs`. Then use them.

### 1. Position relative to the parent pop-up (winit)

- `popup_origin_in_content(chain, content_offset)`: the pop-up's corner measured from the
  root's content area. `chain` holds each `xdg_popup.configure` x, y from the pop-up under the
  root down to the pop-up (each is relative to its parent's window geometry; a pop-up has no
  frame). Sum them and subtract `content_offset` (the root's title-bar offset); an empty chain
  gives `(0, 0)`.
- `popup_request_in_parent(requested, parent_origin)`: `requested - parent_origin` (may be
  negative).
- `popup_positioner_under(parent_origin, content_offset, requested, size)`: `None` is exactly
  `popup_positioner(content_offset, requested, size)`; `Some(origin)` is `popup_positioner((0, 0),
  popup_request_in_parent(requested, origin), size)`.
- Store each pop-up's configured `x, y` (`PopupConfigure::position`) in its `WindowState`
  (`configure_popup` gets them), and keep the parent window id so the chain can be walked.
- `Window::new_popup` (`window/mod.rs`): the transient parent may now be a pop-up window. Then the
  `xdg_popup`'s parent surface is that pop-up's `xdg_surface`, and the positioner is
  `popup_positioner_under(Some(origin of that pop-up), ...)`. A root parent behaves as today.
  A parent window that is gone or dismissed must not panic: fall back to the toplevel
  fallback already there (`warn_popup_fallback`).

### 2. The grab rule (winit)

- `PopupStack<W>` (`push`, `remove`, `topmost`, `may_grab_under`): keep one in `WinitState`. Push a
  pop-up when it is created, remove it when it is dismissed (`PopupHandler::done`), closed or
  dropped.
- `popup_should_grab(serial, parent_ok)`: call `xdg_popup.grab` only when true, with
  `parent_ok = stack.may_grab_under(parent)` (`None` for the root). Otherwise create the pop-up
  without a grab and log at debug level; it must not error, hang or crash. The existing
  `m12-wayland-popup-hang` dismissed handling stays.

### 3. Reposition on resize (winit)

- `popup_resize_action(wm_base_version, old, new)`: `Reposition` when the size changed and the
  version is at least `POPUP_REPOSITION_VERSION` (3), else `Keep`.
- When a pop-up's size changes after creation (`Window::request_inner_size` / the place where the
  size is applied to a pop-up), and the action is `Reposition`, build an `XdgPositioner` of the new
  size from the same placement (same anchor rect and flags as at creation) and call
  `xdg_popup.reposition(&positioner, token)`; answer the compositor's `repositioned` event
  (`PopupHandler::configure` carries the new x, y; keep the stored position current). The version
  is `state.xdg_shell.xdg_wm_base().version()`; check what version sctk binds
  (`XdgShell::API_VERSION_MAX`) and that wayland-protocols has the v3 request. Set the window
  geometry as today.

### 4. egui-winit

- `mark_popup_parent(ctx, title, parent_title)` / `popup_parent_title(ctx, title)`: like
  `mark_popup_title`, keyed by title in the context's temporary data.
- `register_popup_window(ctx, title, id)`: `create_window` calls it for every pop-up it makes
  (after `create_window` on the event loop); a later window with the same title replaces the
  entry.
- `popup_transient_parent(ctx, title)`: the registered window of the title's parent mark, else
  `root_window_id`. `create_window` uses it as `with_transient_parent` for pop-up windows (root
  for everything else, as today).

### 5. Notes

Update `app/vendor/winit/PATCHES.md` and `app/vendor/egui-winit/PATCHES.md` (every changed
function, as the earlier entries do). Keep the `Local patch (Drydock, PATCHES.md)` marker on each
change. The existing tests `popup_positioner_wayland`, `popup_wayland_grab`, `popup_window*`,
`menu_popups`, `submenu_popups` and `menu_close_outside` stay green. The real Wayland run (mutter
harness) is done at sign-off, after `m12-wayland-popup-switch`; `Window::new_popup` cannot be unit
tested, so read the code against the protocol text in the comments you leave.

## Where to look

- `popup_positioner`, `resolve_popup_position`, `PopupGrabSerial`, `PopupDismissed` in
  `app/vendor/winit/src/platform/wayland.rs`.
- `Window::new_popup`, `Window::request_inner_size`, `Window::request_redraw` in
  `app/vendor/winit/src/platform_impl/linux/wayland/window/mod.rs`.
- `WindowState::configure_popup`, `WindowState::content_offset`, `WindowRole::Popup` in
  `.../wayland/window/state.rs`; `PopupHandler for WinitState` in `.../wayland/state.rs`.
- `create_window`, `mark_popup_title`, `is_popup_title`, `root_window_id` in
  `app/vendor/egui-winit/src/lib.rs`.
- `Popup::reposition`, `Popup::xdg_popup` in smithay-client-toolkit (`show Popup smithay-client-toolkit`).
- Tests: `popup_stack_wayland.rs` (the acceptance tests), `popup_positioner_wayland.rs` and
  `popup_wayland_grab.rs` (neighbours).
