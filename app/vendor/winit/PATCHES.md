# Local changes to winit 0.30.13

This directory is the crates.io source of winit 0.30.13 (git e9809ef), wired in with
`[patch.crates-io]` in `app/.cargo/config.toml` and the repository's `.cargo/config.toml`.
The published `Cargo.lock` is left out (cargo ignores it in a dependency). Only the
changes below differ from the published crate; each is marked
`Local patch (Drydock, PATCHES.md)` in the source. Keep this list complete when you change
anything here, and carry it forward when winit is upgraded.

Goal: a dialog or tool window tells the desktop which window it belongs to, so the desktop
keeps it above that window and places it over it. winit's own `with_parent_window` embeds a
child window instead (and is unsupported on Wayland), so it is not reused for this.

- `src/window.rs`: `WindowAttributes` gets a `transient_parent: Option<WindowId>` field,
  `WindowAttributes::with_transient_parent` and the getter `WindowAttributes::transient_parent`.
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new`): when the parent is a window
  of the same event loop, `xdg_toplevel.set_parent` (sctk `Window::set_parent`) before the
  first commit; otherwise a warning and no parent.
- `src/platform_impl/windows/window.rs` (`init`): the transient parent becomes the owner window,
  as `WindowAttributesExtWindows::with_owner_window` does; an explicit owner wins.
- X11: not mapped yet.
