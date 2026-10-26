# m6-dock-ui: panels in the docking columns

Milestone M6 (UI). Crate `modeler`: `src/dock.rs`, `src/app.rs`.

## Goal
Entity Info, Materials, Components and Shadows live in docking panels instead of free-floating windows. The tested part is one widget, `dock::panel_header`, and two small functions; the app wiring around them is yours to build and is checked by hand in the real window.

## Already on main
`DockLayout` and its file (`m6-dock-layout`: `dock`, `float`, `docked`, `floating`, `set_collapsed`, `set_width`, `drop_side`, `drop_position`, `panels_file`), `entity_info::entity_info_panel`, `materials_panel::materials_panel`, `components_panel::components_panel`, `shadows_panel::shadows_panel`. Skeletons for `HEADER_HEIGHT`, `PanelEvent`, `panel_header`, `drop_destination`, `apply_drop` are in `src/dock.rs`.

## Rules for the tested items
- `panel_header(ui, id, collapsed, docked)`: one row, at most `HEADER_HEIGHT` tall, everything centred on it.
  - The title (`title(id)`) is the first item and takes all the room the buttons leave. It senses clicks and drags and is found by the accessible label `Move <title> panel`, role button.
  - When `docked`, then a collapse button, accessible label `Collapse <title>` (`Expand <title>` when `collapsed`). Floating panels have none.
  - Last, a close button, label `Close <title>`.
  - Returns `ToggleCollapsed(id)` / `Close(id)` when a button is clicked. When a drag that started on the title ends, returns `Dropped { id, pos }` with the pointer position at release. A plain click on the title returns nothing.
- `drop_destination(layout, screen, pointer, middles)`: `Dock { side, position }` with `side` from `layout.drop_side` and `position` from `drop_position(middles, pointer.y)`; `Float` when `drop_side` is `None`.
- `apply_drop(layout, id, target, pointer)`: `Dock` calls `layout.dock`. `Float` puts the window's top-left corner at `pointer`, keeping the size it already floated at; a panel that was docked floats at `DEFAULT_WIDTH` x 300.

## App wiring (checked by hand, not by tests)
- `App` holds a `DockLayout`, loaded at start from `dock::panels_file(...)` with `DockLayout::load`, saved with `save` whenever it changes (ignore write errors).
- Each side with open docked panels gets an `egui::SidePanel` of `layout.width(side)`, resizable; saving the dragged width with `set_width`. Panels stack top to bottom in `docked(side)` order, each with `panel_header`, then its content unless collapsed, in a scroll area. Floating panels (`floating()`) are `egui::Window`s at their stored rect, with `panel_header` as the title (no default title bar); moving or resizing them is saved with `float`.
- A `Dropped` event: compute the pointer's column and the vertical middles of that column's other panels, call `drop_destination` then `apply_drop`. While a title is being dragged, tint the column that would receive it.
- `ToggleCollapsed` -> `set_collapsed`; `Close` -> `set_open(id, false)`.
- View menu: a checkable item per panel (title as label) toggling it with `toggle`, and "Reset Panels" calling `reset`. Remove the old per-panel menu items and floating windows (the Shadows window becomes the Shadows panel; keep its keyboard shortcut, if any, toggling it).
- Panel content wiring: Entity Info shows the current selection's `describe` result; Components `ComponentAction::Place` starts `Editor::place_definition` (from `m6-components-place`, if merged; otherwise leave a clear TODO and the click does nothing); Materials and Shadows as they work today.
- The 3D view takes what the panels leave; clicks on panels must not reach it.

## Acceptance
- `cargo test -p modeler --test dock_ui`: header layout, expand/collapse labels, floating header, button events, title drag and click, drop destination table, apply_drop docking and floating.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
