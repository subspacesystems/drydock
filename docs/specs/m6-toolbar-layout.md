# m6-toolbar-layout: toolbars you can arrange

Milestone M6 (UI). Crate `modeler`: `src/toolbars.rs`.

## Goal
The data behind customizable toolbars: which toolbars exist, which tools each holds and in what order, which are shown, and where each sits (docked along one of the window's four edges, or floating). The layout is kept in a small text file between runs. This task is only the layout and its file; a later task draws the toolbars and lets the user drag them.

## Already on main
`src/toolbars.rs`: `Edge`, `Placement`, `Toolbar`, `ToolbarLayout`, `GROUP_NAMES`, `HEADER`, `NEW_TOOLBAR_POS`, `toolbars()`, `reset`, `load`, `save`, `toolbars_file` (done), and stubs for the rest. `chrome::TOOL_GROUPS` gives the tool groups; `icons::TOOLS` lists every tool.

## Layout rules
- `Default`: five toolbars, one per `TOOL_GROUPS` entry, named `GROUP_NAMES[i]`, holding that group's tools in order, all shown, docked along `Top` with slots 0..4.
- Indices into `toolbars()` never change except through `add_toolbar` (appends) and `remove_toolbar` (later ones shift down by one). Methods given an index past the end do nothing.
- Order along an edge is by `slot`. After every method, the slots of all toolbars docked on an edge (shown and hidden) are exactly 0, 1, 2, ... in order, with no gaps or repeats.
- `docked(edge)`: the shown toolbars docked on `edge`, in slot order. `floating()`: the shown floating toolbars, in list order. Hidden toolbars are in neither but keep their placement.
- `set_visible(i, v)`: only the flag changes.
- `dock(i, edge, position)`: shows toolbar `i` and docks it on `edge` so that `docked(edge)[position] == i` (or last when `position` is past the end). The other toolbars on that edge keep their relative order; a hidden one stays just where it was among them (it is inserted before the shown toolbar that was at `position`). Docking on its own edge just reorders. Leaving another edge closes the gap there.
- `float(i, x, y)`: shows it, `Floating { x, y }`, and closes the gap on the edge it left.
- `add_tool(i, tool, at)`: inserts `tool` at `at` (clamped to the end). If the toolbar already has it, it is moved there instead (remove it, then insert at `at` clamped). A tool may sit on several toolbars.
- `remove_tool(i, at)`: removes the button at `at`, if any. Toolbars may be empty.
- `move_tool(i, from, to)`: remove the button at `from` (nothing happens when there is none), insert it at `to` clamped to the end.
- `add_toolbar(name)`: carriage returns and newlines in `name` become spaces, then it is trimmed; empty becomes `"Toolbar"`. If another toolbar has that name, use `"<name> 2"`, `"<name> 3"`, ... (first free). Appends an empty, shown toolbar floating at `NEW_TOOLBAR_POS`, returns its index.
- `remove_toolbar(i)`: deletes it (renumber that edge's slots).

## File format
```
modeler toolbars 1
toolbar 1 top 0 Select
tools select
toolbar 0 left 0 Drawing
tools line circle
toolbar 1 float 10.5 -3 My tools
tools zoom pan
```
- First line is exactly `HEADER`. Then per toolbar, in list order, a `toolbar` line and a `tools` line.
- `toolbar <visible> <edge> <slot> <name>` with visible `1` or `0`, edge `top`/`bottom`/`left`/`right`, slot a whole number; or `toolbar <visible> float <x> <y> <name>` with numbers printed with `{}` (Rust's shortest form that reads back the same). The name is the rest of the line after those fields, with surrounding whitespace removed (spaces inside are kept).
- `tools` followed by the tools' keys, each after one space (just `tools` for an empty toolbar). Keys (`tool_key` / `tool_from_key`, lowercase, exact): select line rectangle circle arc arc3 pushpull move rotate scale tape erase paint orbit pan zoom.
- `to_text` writes exactly that, ending with a newline. For any layout the methods produce, `parse(to_text())` gives it back equal.
- `parse` accepts `\r\n` line ends and blank lines. Text whose first line isn't `HEADER` gives the default layout; a header alone gives an empty layout. A `toolbar` line with a bad field (visible not 0/1, unknown edge, slot not a whole number, x or y not a finite number, no name) is skipped, and so is the `tools` line right after it. A `tools` line that doesn't follow a kept `toolbar` line, and any other line, is ignored. In a `tools` line, unknown keys are skipped and repeats dropped. Afterwards slots are renumbered per edge (by slot, then by file order).

## Acceptance
- `cargo test -p modeler --test toolbar_layout`: default; hide/show; dock to another edge and within one; hidden toolbars keep their place; dock/float show; add/move/remove tools; add/remove toolbars and names; reset; tool keys; the example file both ways; defaults for non-layout text; bad lines skipped; load/save; property tests (random edits keep every rule above and survive the file; `parse` of random text never panics and gives a consistent layout).
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
