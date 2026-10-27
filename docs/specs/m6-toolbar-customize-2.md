# m6-toolbar-customize-2: Customize window as two lists

Milestone M6 (UI), feedback round. Crate `modeler`: `src/toolbars.rs`.

## Goal
The Customize window shows the selected toolbar's tools in an "On the toolbar" list and the other tools in an "Available" list, each row with the tool's icon. Tools are dragged between the lists and within the first one to add, remove and reorder.

## Already on main
`customize_panel`, `CustomizeState`, `ToolbarLayout::{add_tool, remove_tool, move_tool}`, `insertion_index`, `icons::paint`. Skeletons in `src/toolbars.rs`: `CustomizeDrop`, `available_tools`, `apply_customize_drop`.

## Rules for the tested items
- `available_tools(layout, toolbar)`: the tools of `chrome::TOOL_BUTTONS`, in that order, that toolbar `toolbar` does not hold. An out-of-range index holds nothing, so every tool is available.
- `apply_customize_drop(layout, toolbar, tool, drop)` returns true exactly when the layout changed:
  - `OnToolbar(i)`, tool not on the toolbar: insert it at `i` (end when past the end).
  - `OnToolbar(i)`, tool on the toolbar at `at`: it moves so it sits before the entry that was at `i`; the final index is `i` when `i <= at`, else `i - 1`. When that is `at` nothing changes (false).
  - `Available`, tool on the toolbar: remove it from that toolbar only. Tool not on it: false.
  - Out-of-range `toolbar`: false.

## Window rules
- Keep every existing control and its label (`Remove X`, `Add X`, `Move X earlier/later`, toolbar picker, New toolbar, Create, Delete, Reset): the existing `toolbar_customize` tests stay as they are.
- Two headed groups, side by side: "On the toolbar" (the tools in order) and "Available" (`available_tools`). The group headings are labels with exactly those texts.
- Each row starts with a drag handle that is the tool's icon (`icons::paint`, theme colour, `icons::BUTTON_SIZE`) as a drag-sensing widget whose accessible label is `Drag <label>` (e.g. `Drag Line`), then the tool's name and the existing buttons.
- A drag released with the pointer over the "On the toolbar" group drops as `OnToolbar(insertion_index(Edge::Left, row_rects, pos))`, `row_rects` being that list's row rectangles in order (so the vertical position decides). Released over the "Available" group it drops as `Available`. Elsewhere it does nothing. Drop results go through `apply_customize_drop`; `customize_panel` returns true when it changed.
- While dragging, draw a line across the "On the toolbar" group where the drop would land (selection colour).

## Acceptance
- `cargo test -p modeler --test toolbar_customize_lists` and the existing `toolbar_customize`.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
