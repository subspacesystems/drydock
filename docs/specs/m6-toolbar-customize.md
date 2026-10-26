# m6-toolbar-customize: View > Toolbars and the Customize window

Milestone M6 (UI). Crate `modeler`: `src/toolbars.rs`, `src/app.rs`, `src/chrome.rs`.

## Goal
The user can show and hide each toolbar from the View menu, and open a Customize window to add, remove and reorder the tools on a toolbar, make a new toolbar, delete one, and reset everything. The tested part is two widgets in `src/toolbars.rs` that edit a `ToolbarLayout` (the layout operations already exist from `m6-toolbar-layout`); the app wiring is checked by hand in the real window.

Skeletons on main: `ToolbarsMenuEvent`, `CustomizeState`, `toolbars_menu`, `customize_panel`. Tests: `app/crates/modeler/tests/toolbar_customize.rs`.

## `toolbars_menu(ui, layout)`
Called inside a menu's contents. Draws, top to bottom:
- one checkbox per toolbar in list order, labelled with the toolbar's name, checked when it is visible. Clicking toggles `visible` (`set_visible`) and returns `Some(Changed)`;
- a separator;
- a button `Customize...` that returns `Some(Customize)` and changes nothing;
- a button `Reset Toolbars` that calls `layout.reset()` and returns `Some(Changed)`.
Otherwise `None`.

## `customize_panel(ui, layout, state)`
Contents of the Customize window; returns true when it changed the layout. `state.selected` is clamped to the last toolbar first (it stays 0 when there are none). Widgets, each found by its accessible label (kittest `get_by_label`):
- Toolbar picker: one selectable button per toolbar, labelled with its name; clicking sets `state.selected`, no layout change.
- The selected toolbar's tools, in order, one row each: the tool's label (from `chrome::TOOL_BUTTONS`) and three buttons: `Remove <label>`, `Move <label> earlier`, `Move <label> later`. Earlier is disabled on the first row and later on the last. They call `remove_tool`, and `move_tool` by one place.
- Tools not on the selected toolbar: a button `Add <label>` for each, which appends it (`add_tool` at the end). A tool already on the toolbar has no Add button. (A tool may be on several toolbars.)
- A text field labelled `New toolbar name` bound to `state.new_name`, and a button `Create toolbar`: calls `add_toolbar(&state.new_name)`, selects the new toolbar, clears `new_name`. An empty name is fine (the layout names it).
- A button `Delete toolbar`: removes the selected toolbar, then clamps `state.selected`.
- A button `Reset Toolbars`, as in the menu.

## App wiring (checked by hand)
- Menu bar: View gets a Toolbars submenu holding `toolbars_menu`. Layout changes save the layout file as other changes do. `Customize` opens a window titled `Customize Toolbars` (an `egui::Window`, closable) holding `customize_panel`; the `CustomizeState` lives in `App`.
- Hidden toolbars are not drawn; showing one brings it back where it was.

## Acceptance
- `cargo test -p modeler --test toolbar_customize`.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
