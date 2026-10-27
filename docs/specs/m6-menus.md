# m6-menus: menu rows with icons, right-aligned shortcuts and a drawn submenu arrow

Milestone M6 (UI), feedback round. Crate `modeler`: `src/chrome.rs`.

## Goal
Three things Tyrel saw in the menus: the Tools menu has no icons; shortcuts sit right after the label (typed into the label with spaces) instead of lining up at the right edge; the View > Toolbars submenu shows a "?" where its arrow should be (the font has no arrow glyph).

## Already on main
Skeletons in `chrome.rs`: `ItemLayout`, `menu_item_layout`, `menu_item`, `submenu_button` (bodies `unimplemented!`). `menu_bar` / `menu_bar_with_toolbars` draw every row with `ui.button` / `selectable_label` and a label such as `"Save    Ctrl+S"`.

## Rules
- `menu_item_layout(row, has_icon, shortcut_width)`:
  - icon: a 16 x 16 square whose left edge is `row.left() + 4`, centred vertically on the row (none without an icon);
  - `label_left`: `icon.right() + 6` with an icon, `row.left() + 6` without;
  - `shortcut`: a box `shortcut_width` wide and `row.height()` tall, its right edge `row.right() - 6`, top at `row.top()`.
- `menu_item(ui, icon, label, shortcut, selected)` draws one full-width row:
  - the row is the clickable widget; its accessible label is exactly `label` (no shortcut in it), reported as a selectable button with `selected`, so tests find it by label and read its toggled state;
  - hover and `selected` get a background like `icons::icon_button`; the icon (if any) is drawn with `icons::paint` in the text colour;
  - the row is at least as wide as icon + label + a 24 px gap + shortcut + padding, and fills the available width, so in one menu every shortcut ends at the same x;
  - a non-empty `shortcut` is its own text node (a `Label`, not selectable) placed at `ItemLayout::shortcut`, found by its text; an empty one adds no node;
  - the row is disabled with the `Ui` (callers wrap it in `add_enabled_ui`); a disabled row does not report clicks.
- `submenu_button(ui, label, add_contents)` is a submenu row (opens on click or hover like `ui.menu_button`) whose accessible label is exactly `label` (no trailing space) and whose arrow is a small triangle drawn with shapes at the right edge, never a text glyph. It returns what `ui.menu_button` does.
- Every row of `menu_bar_inner` goes through `menu_item` (View > Toolbars through `submenu_button`):
  - the label has no shortcut in it; the shortcut goes in the shortcut argument ("Ctrl+N" ... as today, e.g. `Undo Line` with `Ctrl+Z`, tool shortcuts from `tool_shortcut`);
  - the Tools menu passes `Icon::Tool(tool)`, selected when it is the current tool; the other menus have no icons;
  - checked rows (Axes, panels, Shadows) pass `selected`;
  - disabled rows (Undo / Redo / Cut / Copy / Paste / Erase / Make... / Explode / Flip with nothing to act on) stay disabled and keep their shortcut text;
  - the name prompt's OK and Cancel buttons are not menu rows and stay as they are.
- Behaviour of every action is unchanged.

## Acceptance
- `cargo test -p modeler --test menus_items` (new): layout numbers; label, click, selected state and shortcut node of `menu_item`; shortcuts in File and Tools share a right edge; no button label contains four spaces; Edit and View shortcuts present; Tools icons rows still select; disabled rows do nothing; the Toolbars submenu has no arrow glyph and opens.
- The existing `menus`, `chrome` tests (labels there lost their embedded shortcuts; `chrome.rs`'s `choose` prefers an exact label), `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
