# m6-menus-2: menus sized to their content, icon column, check marks, View regrouped

Milestone M6 (UI), feedback round 3. Crate `modeler`: `src/chrome.rs`, `src/toolbars.rs` (`toolbars_menu` only).

## Goal
Make the menus look like desktop menus:
1. Every menu is as wide as its widest row needs and no wider. Today each row takes the popup's whole available width, so menus come out several hundred points wide with a big gap before the shortcuts.
2. Every row leaves an icon-sized column before its label, so labels line up whether or not a row has an icon.
3. Items that show or hide something (View > Axes and the panels, View > Toolbars > each toolbar) show a check mark in that column when on, instead of highlighting the whole row. The current tool in the Tools menu gets a box around its icon instead of a row highlight.
4. The View menu groups show/hide items apart from the rest.
5. Items that open a window end with "...".

## Already on main (skeleton)
`chrome::menu_check_item(ui, label, checked) -> Response` with an `unimplemented!()` body.

## Rules
- **Row geometry.** `menu_item_layout(row, has_icon, shortcut_width)`: `label_left` is always `row.left() + 26` (4 margin, 16 icon, 6 gap), with or without an icon. The icon rect and shortcut rect are unchanged.
- **Width.** A row needs `26 + label width + 6`, plus `24 + shortcut width` when it has a shortcut (a submenu row adds 24 for its arrow instead). `menu_item`, `menu_check_item` and `submenu_button` all size the same way: while egui sizes the popup (`ui.is_sizing_pass()`, the first frame a popup is open) allocate exactly what the row needs; on later frames allocate `ui.available_width().max(needed)`. The popup then comes out as wide as its widest row, and every row spans it. Don't use a fixed width, and don't use `spacing.menu_width`. The Edit menu must widen for a long "Undo ..." label and narrow again for a short one.
- **Highlights.** Hovering a row fills it with `weak_bg_fill`, as now. Being selected or checked never fills the row.
- **`menu_check_item`.** Same row as `menu_item` without an icon or shortcut. `WidgetInfo::selected(WidgetType::Checkbox, enabled, checked, label)`, so screen readers and tests see a check box with its state. When `checked`, draw a check mark (a two-segment polyline, about 1.8 pt, text color) inside the icon column (`menu_item_layout(row, true, 0.0).icon`). Nothing in that column when unchecked.
- **Selected `menu_item` with an icon** (the Tools menu's current tool): a box around the icon (the icon rect expanded by up to 4 pt, selection-colored fill and/or 1 pt stroke), drawn under the icon. The row is not filled.
- **View menu**, top to bottom, with separators where `|` is:
  projection, Zoom Extents | the seven standard views | Hide/Show Rest of Model | **Axes** (check) | **Entity Info, Materials, Components, Shadows, Texture Position** (checks) | **Toolbars ▸, Reset Panels** | **Model Info...**.
- **Toolbars submenu** (`toolbars::toolbars_menu`): one `menu_check_item` per toolbar (clicking flips its visibility, as now), separator, then `Customize...` and `Reset Toolbars` as `menu_item` rows.
- **Ellipses.** "About modeler..." (Help) and "Model Info..." (View) gain "..."; the actions are unchanged. File > Open..., Save As..., Edit > Make Component..., Make Group... and Toolbars > Customize... keep theirs.

## Notes
- The test build has no egui default fonts (the app ships its own), so tests that measure text load `look::font_definitions(None)` first.
- A throwaway implementation (about 150 lines in `chrome.rs`, 8 in `toolbars.rs`) passes every test. The sizing-pass rule above is what made the widths come out right.

## Acceptance
- `cargo test -p modeler --test menus_fit` (width per menu, Undo widening, icon column, check items, tool icon box, View check items and groups, Toolbars submenu, ellipses).
- Updated in this spec's commit: `menus_items` (label after the icon column; its menus load the app fonts), `menus` and `status` (the new "About modeler..." and "Model Info..." labels).
- All other existing tests unchanged, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
