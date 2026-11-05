# m9-linux-titlebar: GNOME-style title bar, app id and window icon on Linux

On Ubuntu (Wayland) the window's title bar and its minimise, maximise and close buttons are drawn by the app and look unlike other windows, show no title text, and the dock shows a gear. GNOME offers no server-side decorations, so winit draws its own frame; its `wayland-csd-adwaita` feature (the GNOME look, with the title) is off because eframe builds winit with default features off. The title text is already right: `Document::title` is sent with `ViewportCommand::Title` in `app.rs`.

Already on main (skeleton commit): the dependency line in `crates/drydock/Cargo.toml`, the updated `Cargo.lock` (sctk-adwaita and its crates, all MIT / Apache-2.0 / BSD, inside `deny.toml`; see `docs/adr/008-linux-window-decorations.md`), `APP_ID`, `ICON_PNG`, `assets/drydock-256.png`, and `window_icon()` as a stub.

## To do
1. `window_icon::window_icon()`: decode `ICON_PNG` with the `image` crate (already a dependency, `png` feature) into `eframe::egui::IconData { rgba, width, height }`. Square, RGBA8, 256 px. Do not use egui's default icon.
2. `main.rs`, the `ViewportBuilder` in `main()`: add `.with_app_id(APP_ID)` and `.with_icon(window_icon())` (import from `drydock::window_icon`). Keep the title, size and drag-and-drop settings. The icon and id apply on every OS; on Windows the `.exe` resource icon is unchanged.
3. Do not change `Cargo.toml`, `Cargo.lock`, `deny.toml`, `APP_ID`'s value or the PNG. If `cargo build` wants to change `Cargo.lock`, the winit version in the lock no longer matches the dependency line: stop and say so in the commit message.

## Behaviour
- `window_icon()` returns a square image of 256 px whose `rgba.len() == width * height * 4`, with the transparent rounded corner and the navy tile of `make-icon.py` (not blank, not egui's icon).
- The window is created with app id `com.subspacesystems.Drydock`: Wayland `app_id` and X11 `WM_CLASS` carry it, so the desktop entry of the next task (`m9-linux-desktop-entry`) matches the window and the dock shows our icon.
- eframe's `persistence` feature is off in the workspace, so the app id moves no stored window state. Confirm this (grep the workspace `Cargo.toml` for the eframe features) and say so in the commit message.
- Nothing else in the UI changes. Tyrel checks the Wayland title bar on Ubuntu; `gui-session` runs Xvfb with no window manager and cannot show decorations, so the checks there are: the app starts and renders, and (if `xprop` exists) `xprop` on the window shows `WM_CLASS` with the app id and `_NET_WM_ICON`.

## Tests (already written, do not edit)
`crates/drydock/tests/linux_titlebar.rs`: the lock lists `sctk-adwaita` and the single `winit` entry depends on it; the crate asks for the feature in a Linux-only section without default features; the allow list in `deny.toml` did not grow; `APP_ID` is a valid reverse-DNS desktop id; the icon is square RGBA of a sensible size with the expected corner, tile and cube-top pixels; `ICON_PNG` equals the `.ico`'s 256 px frame; `main.rs` calls `.with_app_id(APP_ID)` and `.with_icon(window_icon())`.

## Where to look
- `show window_icon` and `show APP_ID` (`src/window_icon.rs`); `ICON_PNG` in the same file.
- `show main` in `src/main.rs` (the `eframe::NativeOptions` / `ViewportBuilder` near the end).
- `eframe::egui::IconData` and `ViewportBuilder::with_icon` / `with_app_id` (`harness/bin/api-map egui`).
- `image::load_from_memory_with_format`; other uses of `image` in the crate (`texture_handles.rs`) for the call style.
- `assets/make-icon.py` (writes both icon files), `docs/adr/008-linux-window-decorations.md`.
