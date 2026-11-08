# m11-dialog-sun: Shadows and Geolocation in their own windows

Part of. `m11-dialog-window` added the helper (`dialog_window`) and moved Model Info and
About to it. This task does the same for the two sun windows in `sun_windows.rs`.

## What to build

- In `SunWindows::show`, replace both `egui::Window::new(..)` wrappers with `dialog_window`:
  - **Shadows**: `DialogWindow { title: "Shadows", size: .. }`.
  - **Geolocation**: `DialogWindow { title: "Geolocation", size: .. }`.
  Pick sizes that fit the content (check with a screenshot in the real window; the Geolocation
  one holds the globe).
- Both stay **modeless**: nothing blocks the main window, and while Shadows is open a slider
  drag still previews live in the viewport (`apply_shadows_event` is unchanged). Each viewport
  callback runs with the shared `Context`, so `shadows_panel`, `geo_fields_with_globe` and
  `button_row` need no change.
- Closing: the helper clears its `open` flag on title-bar close or Escape; the Close button
  (and Apply, for Geolocation) clear it as today. Keep the existing cleanup when the window
  goes away: Shadows drops the preview; Geolocation drops `form` and `globe`. Copy `open`
  into a local before the call and write it back after, because `content` borrows `self`.
- `content` is `FnOnce` and borrows `self.form`, `self.globe`, `settings`, `event`, `applied`,
  `close`: collect results in locals as the current code does.
- Test hook (the one `m11-dialog-window` added in `app.rs`, `DRYDOCK_OPEN_DIALOG`): also
  accept `shadows` (sets `sun_windows.shadows_open`) and `geolocation` (sets `geo_open`).
- Window > Shadows and Window > Geolocation (`app.rs`, where `geo_open` is set) keep working
  as they are.
- No new dependency.

## Acceptance tests

`tests/dialog_sun.rs` (do not edit):
- Embedded: Shadows shows a window titled "Shadows" with its panel; Escape closes it and
  drops the preview; Geolocation likewise, and its form is rebuilt from the model when
  reopened.
- Real window (Xvfb, xdotool, xprop, `MODELER_GUI_TESTS=1`; skips itself otherwise): starts
  the app with `DRYDOCK_OPEN_DIALOG=shadows` (then `geolocation`), finds a second X window with
  that title whose `WM_TRANSIENT_FOR` is the main window, checks the main window is still
  there, presses Escape in the dialog and checks it goes.

The existing kittest tests (`sun_windows.rs`, `dialogs.rs`, `geo_globe.rs`) must pass unchanged.
Run `harness/bin/run-tests -p drydock` at the end with Xvfb and `MODELER_GUI_TESTS=1`.

## Where to look

Use `show <name>`:
- `SunWindows::show`, `SunWindows::apply_shadows_event`, `SunWindows::close` in `sun_windows.rs`.
- `dialog_window`, `DialogWindow`, `button_row`, `DialogButton` in `dialog.rs`.
- `shadows_panel::shadows_panel`, `geo_panel::geo_fields_with_globe`, `geo_panel::parse_geolocation`.
- In `app.rs`: where `DRYDOCK_OPEN_DIALOG` is read, and the `geo_open` / `shadows_open` setters.
- Test helpers: `harness`, `state` in `tests/sun_windows.rs`; `windows`, `wait_for`, `Kill`
  in `tests/dialog_window.rs` (copy them).
