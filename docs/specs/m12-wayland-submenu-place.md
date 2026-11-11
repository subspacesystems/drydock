# m12-wayland-submenu-place: a submenu opens beside its menu on Wayland

Sign-off 6 on GNOME: View > Toolbars got `xdg_positioner.set_anchor_rect(189, 579, 1, 1)`
relative to the View pop-up, which was configured at (74, 61) in the main window's geometry
(content offset (0, 35)). It should be about (266, 605): the Toolbars row's right edge and top
inside View. So the submenu overlaps its menu, and a click on its rows can land in the main
window.

Cause: Wayland reports no window positions, so the app (`submenu_button` in `chrome.rs`) can only
give the submenu's place in its menu pop-up's own points, and it does. The vendored winit
(`Window::new_popup`) then treats a request for a pop-up-parented pop-up as if it were in the
root's content coordinates and subtracts the parent's origin, which is the origin already
subtracted once by the menu's own placement. (74, 26) is removed twice.

## Decision: the contract changes on the winit side

On Wayland the position (`WindowAttributes::with_position`) of a pop-up whose parent is a
pop-up is **in that parent pop-up's coordinates**: the parent's top-left is (0, 0). That is all
the app can know there, and the compositor may slide the parent anyway. Root-parented pop-ups
are unchanged (relative to the root's content area, plus the title-bar offset). X11 and Windows
are untouched (absolute positions).

Changes (the app side stays as it is):

1. `popup_positioner_under(Some(_), content_offset, requested, size)` in
   `vendor/winit/src/platform/wayland.rs`: the anchor is the 1-px rect at `requested` itself
   (no subtraction, no title-bar offset), with the same offset (0, 0) and flags as
   `popup_positioner` (slide and flip on both axes, so the compositor keeps it on screen).
   `None` is exactly `popup_positioner(content_offset, requested, size)` as before. Keep the
   signature (the `Some` payload is no longer used; say so in its doc comment).
2. In `Window::new_popup` (`wayland/window/mod.rs`, the "Local patch" block that builds
   `parent_origin`): drop the chain walk. When the parent is a pop-up pass `Some(..)`
   (e.g. `Some((0, 0))`), else `None`.
3. Remove `popup_origin_in_content` and `popup_request_in_parent` (and anything that becomes
   unused as a result, e.g. `WindowState::popup_position` / `popup_parent` if nothing else
   calls them; the build uses `-D warnings`). The tests no longer import them.
4. Docs: update `vendor/winit/PATCHES.md` (the pop-up stack paragraph that describes these
   functions), the doc comments of `popup_positioner_under`, `WindowAttributes::with_popup`
   if it mentions the chain, and in the app `popup_root_inner` and `PopupWindow::position`
   (`crates/drydock/src/popup.rs`): on Wayland a pop-up with a `parent` is positioned in that
   parent pop-up's points, not the root's.

Do not change `chrome.rs` behavior; the new hook `submenu_row_rect` is already on main.

## Tests (already written; do not edit)

- `crates/drydock/tests/popup_stack_wayland.rs`: the position-conversion tests rewritten for the
  new contract, including the exact sign-off numbers (View at (74, 61), request (266, 605) must
  give an anchor of (266, 605), not (192, 579)).
- `crates/drydock/tests/popup_submenu_place.rs`: a kittest under `force_wayland_windows` that
  opens View > Toolbars and checks the `ViewportBuilder` position the Toolbars pop-up was created
  with equals the row's (right, top) in View's points, also when a root inner rect is reported.
  It passes before the change (the app was already right); it guards against regressions.

Sign-off 7 reruns the GNOME steps and expects Toolbars configured at x near View's width
(y slid up to fit an 800-px screen) and a click on Drawing that leaves both pop-ups alive.

## Where to look

- `popup_positioner`, `popup_positioner_under`, `popup_origin_in_content`,
  `popup_request_in_parent`, `resolve_popup_position`, `PopupRect` (`vendor/winit/src/platform/wayland.rs`)
- `Window::new_popup` (`vendor/winit/src/platform_impl/linux/wayland/window/mod.rs`, ~400-445),
  `WindowState::popup_position`, `WindowState::popup_parent` (`.../wayland/window/state.rs`)
- `vendor/winit/PATCHES.md` (pop-up sections)
- `popup_root_inner`, `PopupWindow` (`crates/drydock/src/popup.rs`)
- `submenu_button`, `submenu_popup_position`, `submenu_row_rect` (`crates/drydock/src/chrome.rs`)
- Test helpers: `open_toolbars`, `toolbars_request` in `tests/popup_submenu_place.rs`
