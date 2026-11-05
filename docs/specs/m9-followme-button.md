# m9-followme-button: Follow Me gets key F, a toolbar button and a menu icon

Tyrel: Follow Me can only be reached through Tools > Follow Me, with no key, icon or
toolbar button. Make it a tool like Offset. The icon is already drawn
(`icons.rs`, `Icon::Tool(Tool::FollowMe)`), `toolbars::tool_key`/`tool_from_key` already
know "followme", and the tool itself is unchanged.

## Changes (all in `crates/drydock/src`)

- `chrome::TOOL_BUTTONS`: 17 -> 18 entries. Add `(Tool::FollowMe, "Follow Me")` right after
  Offset.
- `chrome::TOOL_GROUPS`: add `Tool::FollowMe` to the modify group right after `Tool::Offset`
  (this also puts it in the default toolbar and the Tools menu, which draws groups with icon and key).
- `icons::TOOLS`: 17 -> 18, same order as `TOOL_BUTTONS` (FollowMe after Offset).
- `chrome::tool_shortcut(Tool::FollowMe)` returns `"F"` (today `""`).
- `input::key`: plain `"F"` gives `Action::SelectTool(Tool::FollowMe)`. Shift, Ctrl and Alt
  with F stay unbound. Check nothing else in `input::key` or `app.rs` already uses plain F.
- Tools menu (`chrome::menu_bar`): delete the separate `menu_item(ui, None, "Follow Me", ...)`
  and its separators after the groups loop; keep one separator before "Fit Check". The groups
  loop now draws Follow Me once, with icon and "F". The menu must contain exactly one
  "Follow Me" item.
- Saved toolbar layouts: `ToolbarLayout::parse` (so `load`) must add the new button to a layout
  written before it existed. After parsing, if no toolbar holds `Tool::FollowMe` and some
  toolbar holds `Tool::Offset`, insert `FollowMe` right after `Offset` in the first toolbar
  that holds Offset. Otherwise change nothing (a layout with no Offset is returned as written;
  `to_text` output for the existing example layouts stays byte-identical). Do not change `HEADER` or
  the file format. Known limit: a user who removes Follow Me from every toolbar but keeps Offset gets it back
  at the next load; accept that.
- The guide pages are already updated on main (`docs/guide/shortcuts.md`, `extrude.md`); do
  not edit them.
- The status hint, the editor and `follow_tool.rs` are unchanged.

## Tests (already in the spec commit; do not edit)

- `tests/followme_ui.rs`: button after Offset, `TOOLS` order, group and default toolbar, toolbar
  click selects the tool, `F` key and unbound modifiers, one Follow Me menu item with its key shown, old
  layout migration.
- `tests/offset_ui.rs`, `chrome.rs`, `toolbar_layout.rs`: counts and orders updated to 18 tools.
- Existing generic tests (`menus.rs`, `menus_items.rs`, `icons.rs`, `icons_2.rs`,
  `guide_coverage.rs`) pick the new tool up through `TOOL_BUTTONS`.

Run `harness/bin/run-tests -p drydock`.

## Where to look

- `show TOOL_BUTTONS`, `show TOOL_GROUPS`, `show tool_shortcut`, `show menu_bar` (the Tools menu part), all in `chrome.rs`.
- `show TOOLS` in `icons.rs`.
- `show key` in `input.rs` (the `"I" if plain` arm is the model).
- `show ToolbarLayout::parse`, `show tool_from_key` in `toolbars.rs`.
- Tests: `tools_harness` in `tests/followme_ui.rs`; `toolbar_harness` in `tests/chrome.rs`.
