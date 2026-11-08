# m11-dialog-app-id: dialog windows and floating panels carry Drydock's app id and icon

Tyrel (Ubuntu GNOME): dialogs show up in the dock and window switcher as separate
unknown apps. Only the main window sets the application id and icon; child viewports set
neither, so GNOME cannot match them to the desktop entry (X11: no `WM_CLASS` either).

## Behaviour
- `window_icon::with_app_identity(builder)` (already on main) adds `APP_ID` and `window_icon()`
  to a `ViewportBuilder`. Every window Drydock opens goes through it.
- The main window (`main.rs`, the `NativeOptions` viewport), the real-window dialog viewport
  (`dialog_window_impl`, so plain dialogs and modal prompts) and the floating panel viewport
  (`floating_panel_window`) are built through it. Replace main.rs's direct
  `.with_app_id(APP_ID).with_icon(window_icon())` by the helper; drop imports that become unused.
- Embedded mode (kittest) is unchanged.
- Out of scope: GNOME Wayland still lists dialogs under Drydock's icon; do not try to hide them.

## Tests (already written, do not edit)
- `tests/dialog_app_id.rs`: builders of a dialog, a modal prompt and a floating panel carry
  `app_id == APP_ID` and an icon.
- `tests/dialog_app_id_x11.rs`: under Xvfb with `DRYDOCK_GUI_TESTS=1`, `WM_CLASS` of a dialog and
  a floating panel equals the main window's. (Skips without Xvfb.)

## Where to look
- `show with_app_identity`, `show APP_ID`, `show window_icon` (window_icon.rs)
- `show dialog_window_impl` (the `ViewportBuilder::default()` at the real-window branch)
- `show floating_panel_window` (the `let mut builder`)
- main.rs `NativeOptions` in `main`
