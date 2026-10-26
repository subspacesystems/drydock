# m6-toolbar-ui: drawing a toolbar

Milestone M6 (UI). Crate `modeler`: `src/toolbars.rs`, `src/app.rs`, `src/chrome.rs`.

## Goal
Tools show as icon buttons in toolbars drawn from the toolbar layout, instead of the text buttons down the left side. The part with tests is one widget, `toolbars::toolbar_strip`, and two small functions; the app wiring around it is yours to build and is checked by hand in the real window.

## Already on main
`ToolbarLayout` (`m6-toolbar-layout`), `icons::icon_button` and `Icon::Tool` (`m6-icons`), `chrome::TOOL_BUTTONS` (tool and label), `chrome::tool_shortcut`. The skeletons for the items below are in `src/toolbars.rs`: `EDGE_ZONE`, `GRIP_LENGTH`, `ToolbarEvent`, `drop_target`, `tool_tooltip`, `toolbar_strip`.

## Rules for the tested items
- `tool_tooltip(tool)`: the label from `chrome::TOOL_BUTTONS`, a space, then the shortcut from `chrome::tool_shortcut` in parentheses: `Line (L)`, `3-Point Arc (Shift+A)`.
- `drop_target(pos, window)`: the distance from `pos` to each edge of `window` (a point outside the window counts as distance 0 to an edge it is beyond). The nearest edge whose distance is at most `EDGE_ZONE` wins; on a tie, the first of Top, Bottom, Left, Right. No edge in range: `None`.
- `toolbar_strip(ui, toolbar, index, current, vertical)`:
  - Lays out a grip, then one `icons::icon_button` per tool in `toolbar.tools` order, label the tool's label in `chrome::TOOL_BUTTONS`, tooltip `tool_tooltip`, selected when the tool is `current`. Horizontal (`vertical == false`): left to right; vertical: top to bottom. Buttons touch no more than the `ui` item spacing apart and share one row (or column).
  - The grip is first: `GRIP_LENGTH` along the toolbar and `BUTTON_SIZE` across it, found by the accessible label `Move <toolbar name> toolbar`, role button, and sensing drags. It draws something visible (a few dots or lines in the theme's text color) so it reads as a handle.
  - Returns `Select(tool)` when a button is clicked. When a drag that started on the grip ends, returns `Dropped { index, pos }` with the pointer's position when it let go. A plain click on the grip returns nothing.
  - An empty toolbar shows just its grip.

## App wiring (checked by hand, not by tests)
- Remove the left `toolbar` panel and `chrome::toolbar`'s use. Keep `TOOL_BUTTONS` and `TOOL_GROUPS`.
- `App` holds a `ToolbarLayout`, loaded at start from `toolbars::toolbars_file(...)` with `ToolbarLayout::load` and saved with `save` whenever it changes (ignore write errors).
- Under the menu bar, one `Panel` per edge that has shown docked toolbars, holding them in `docked(edge)` order (a row on top and bottom, a column on left and right). Floating toolbars (`floating()`) are `egui::Area`s at their position, with a thin frame.
- A `Dropped` event docks the toolbar on `drop_target(pos, window)` at the position among that edge's toolbars its pointer is nearest (just the end is acceptable for the first version), or floats it with its top-left corner at `pos` when there is no target. A `Select` applies `Action::SelectTool`.
- Drawn toolbars must not cover the viewport: the 3D view takes what the panels leave.

## Acceptance
- `cargo test -p modeler --test toolbar_ui`: tooltips, the drop-target table, strip order, directions, sizes, tooltips on hover, selected state, click and grip-drag events, empty toolbar.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
