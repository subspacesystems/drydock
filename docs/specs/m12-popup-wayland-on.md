# m12-popup-wayland-on: pop-up windows on for Wayland

Fix found in the M12 sign-off. `m12-winit-popup-wayland` made the local winit patch create real
`xdg_popup`s (see `app/vendor/winit/PATCHES.md`, "Pop-up windows" on Wayland), but
`popup_supported_on("linux", true)` still returns false, so on Wayland every menu opens inline
inside the window. Turn it on, and make the position rule explicit.

## Behaviour

1. `popup::popup_supported_on`: "linux" is supported on both X11 and Wayland (the `wayland`
   argument no longer matters for Linux); "windows" yes; anything else no. Update its doc
   comment (it still says Wayland waits for the winit task).
2. Positions. On Wayland the patch wants a pop-up's position relative to the parent's content
   area, not screen coordinates. Everywhere else it is screen coordinates (root inner
   top-left plus the local point). New `popup::popup_root_inner(root_inner, wayland)`: `None`
   when `wayland`, else `root_inner` unchanged. `menu_popup_position`, `submenu_popup_position`
   and `context_popup_position` already add `root_inner.min` when it is `Some`, so feeding them
   `popup_root_inner(..)` makes them content-relative on Wayland with no other change.
3. Wire it: every place that reads the root's `inner_rect` to compute a pop-up position
   (menu title in `chrome.rs`, submenu row, `open_context_menu`) passes it through
   `popup_root_inner(.., wayland)`, where `wayland` is a new small helper
   `popup::wayland_session()` (the existing environment check inside `popup_supported`, moved
   out and reused by it: `WAYLAND_DISPLAY` non-empty and `WINIT_UNIX_BACKEND` not "x11").
   Inline pop-ups in `popup.rs` (`popup_window_inline`) subtract the root's position;
   they must keep working as they do now (UI tests are embedded, so `root_inner` is
   `None` or the test's own value there).
4. Check, without changing unless wrong: egui-winit's `create_window` gives winit the root
   window as the transient parent for a window whose title was marked with
   `mark_popup_title` (it does: `with_transient_parent(root)` and `with_popup`). A submenu's
   parent is the main window too; nested `xdg_popup` is out of scope.
5. The close rules from `m12-menu-close-outside` must keep working: on Wayland the compositor
   ends the grab on an outside click and the pop-up gets a close request, which
   `popup_window_window` already turns into `*open = false`. Do not change that path.

## Tests

`tests/popup_wayland_on.rs` (new) and the changed assertion in `tests/popup_window.rs`
(`platform_support`). No Wayland compositor is available here, so the window itself is
checked by Tyrel on Ubuntu GNOME in the milestone's try round. Run
`harness/bin/run-tests -p drydock`; do not run the GUI tests locally.

## Where to look

- `show popup_supported_on`, `show popup_supported`, `show popup_mode`, `show popup_root_inner`
  (skeleton, `unimplemented!`)
- `show menu_popup_position`, `show submenu_popup_position` (and the code that calls them in
  `chrome.rs`), `show context_popup_position`, `show open_context_menu`
- `show popup_window_window`, `show popup_window_inline` (read only)
- `app/vendor/egui-winit/src/lib.rs`: `create_window`, `is_popup_title` (read only);
  `app/vendor/winit/PATCHES.md` (read only)
- Tests: `tests/popup_wayland_on.rs`, `tests/popup_window.rs::platform_support`
