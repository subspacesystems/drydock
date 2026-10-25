# m5-app-shadows-ui: the Shadows panel and View > Shadows

Milestone M5 (Sun). Crate `modeler`: `src/shadows_panel.rs` (skeleton on main, fill in the bodies) and `src/chrome.rs` (the View menu). `Action::ToggleShadows`, `Effect::ToggleShadows` and `MenuState::shadows` are already on main; the app shows the window itself in a later task, so don't touch `app.rs`.

## Goal
A panel that edits a `ShadowSettings` value (`modeler_kernel::sun`) and a View menu item that shows or hides it. The panel doesn't write to the model: it changes the copy it is given and says whether the app should preview it (a slider is mid-drag) or record it (one undo step).

## Helpers (pure)
- `minutes_of_day(time)`: hour × 60 + minute.
- `with_minutes(time, m)`: same date, clock set to `m` minutes after midnight; `m` above 1439 gives 23:59.
- `with_day_of_year(time, day)`: same year and clock, on day `day` of the year; `day` is clamped to 1..=365 (366 in a leap year, Gregorian rule).
- `time_label` gives "HH:MM", `date_label` gives "YYYY-MM-DD" (zero padded), `percent_label(share)` gives the share as a rounded whole percent such as "80%".

## Panel
`shadows_panel(ui, settings) -> ShadowsEvent`. Widgets, top to bottom, with these exact labels:
- heading "Shadows";
- checkboxes "Show Shadows" (`shadows`), "Sun Shading" (`sun_shading`), "Shadows on Ground" (`on_ground`);
- slider "Time of Day": whole minutes 0..=1439, edits the clock through `with_minutes`; a label beside it shows `time_label`;
- slider "Date": days 1..=366 of the year, edits the date through `with_day_of_year`; a label beside it shows `date_label`;
- sliders "Light" and "Dark": 0..=1 on `light` and `dark`, each with a `percent_label` label beside it.
Build every slider with `.show_value(false)` so each is one accessible node under its label (the value is the separate label).

Result: `Commit` when a checkbox is clicked, or a slider changed without being dragged (keyboard), or a drag ended; `Preview` while a slider changes during a drag; otherwise `None`. A frame with no change is `None`.

## View menu
Below Axes in the View menu add a selectable item "Shadows", checked when `MenuState::shadows` is set, that yields `Action::ToggleShadows` and closes the menu.

## Acceptance
`cargo test -p modeler --test shadows_panel`; all other tests unchanged; `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`.
