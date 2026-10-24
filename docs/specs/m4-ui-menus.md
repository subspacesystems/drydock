# m4-ui-menus: Exit, recent files, Tools menu, About with license

Tyrel's UI list, items 1, 8, 10, 14 and 15. App crate only (`modeler`).

## What the user sees

Menu bar, left to right: **File, Edit, View, Tools, Help**.

**File**, top to bottom, with a separator line between the groups:

1. `New    Ctrl+N`, `Open...    Ctrl+O`
2. `Save    Ctrl+S`, `Save As...    Ctrl+Shift+S`
3. The recent files (only when there are any), most recent first, text from
   `chrome::recent_labels`. Hovering one shows its full path.
4. `Exit`

Clicking a recent file returns `Action::OpenRecent(index)` (index into
`MenuState::recent`). `Exit` returns `Action::Exit`. Both close the menu.

**Tools** lists every tool, in `chrome::TOOL_GROUPS` order, with a separator between
groups. Each item's text is the toolbar label (`TOOL_BUTTONS`), four spaces, then
`chrome::tool_shortcut(tool)`, e.g. `Push/Pull    P`, `3-Point Arc    Shift+A`. The
current tool (`MenuState::tool`) is drawn selected (a selectable item: its accessibility
node reports toggled true; the others report not toggled). Clicking an item returns
`Action::SelectTool(tool)` and closes the menu. The toolbar can later hide tools; this
menu always lists them all.

**Help**: `About modeler` as now. The About window shows `chrome::about_lines()`, one
label per line, then the Close button.

Edit and View keep their items and separators.

## `chrome::recent_labels(paths)`

One string per path, in order: `"{n}  {name}"` with `n` counting from 1, two spaces,
and `name` the path's file name (the whole path, displayed, when it has no file name).
When two or more paths in the list have the same file name, each of those gets
` ({folder})` appended, where `folder` is the file name of its parent folder (left off
when there is none). Paths with a unique file name get nothing appended.

## `chrome::about_lines()`

1. `chrome::about_text()` (name, version, commit) as now.
2. `License: ` followed by the crate's license expression (`CARGO_PKG_LICENSE`, today
   `GPL-3.0-or-later`).
3. `Copyright (c) 2026 Subspace Systems`
4. `Contributors:`
5. Then each line of `chrome::CONTRIBUTORS` (the repository's `CONTRIBUTORS` file),
   trimmed, skipping empty lines and lines that start with `#`.

## `recent` module

`RecentFiles` holds at most `MAX_RECENT` (8) paths, most recent first, no two equal.

- `add(path)`: make `path` absolute with `std::path::absolute` (keep it as given if that
  fails), remove any equal entry, insert at the front, drop entries past `MAX_RECENT`.
- `remove(path)`: make `path` absolute the same way and drop the equal entry, if any.
- `parse(text)`: one path per line; a trailing `\r` is dropped; empty lines are skipped;
  lines are kept as written (not made absolute); a later duplicate of an earlier line is
  skipped; only the first `MAX_RECENT` distinct paths are kept.
- `to_text()`: each path followed by `\n`, in order. A path that isn't valid Unicode is
  left out. `parse(&list.to_text()) == list` for lists built with `add`.
- `load(file)`: `parse` of the file's text; an empty list when the file is missing,
  can't be read, or isn't UTF-8.
- `save(file)`: create the file's folder (and its parents) when missing, write the text
  to a temporary file in the same folder, then rename it over `file`. Errors are returned.

`config_dir(var)` reads environment variables only through `var` (tests pass a map):

1. `MODELER_CONFIG_DIR`, when set and not empty: that path as is.
2. On Windows (`cfg!(windows)`): `APPDATA`, when set and not empty, joined with `modeler`.
3. Otherwise `XDG_CONFIG_HOME`, when set, not empty and absolute, joined with `modeler`.
4. Otherwise `HOME`, when set and not empty, joined with `.config/modeler`.
5. Otherwise `None`.

`recent_file(var)` is `config_dir(var)` joined with `recent-files` (already on main).

## App wiring (`app.rs`, checked in the real window at review)

- `App::new` loads the list from `recent_file(|name| std::env::var_os(name))` (empty
  when that is `None`) and remembers the file.
- Each time a document finishes loading from a file (including the file given on the
  command line), and after each successful Save As, `add` its path and `save` the list.
  A failed save of the list is ignored (no message). New documents add nothing.
- `Effect::OpenRecent(index)`: an index past the list does nothing. When the file no
  longer exists, `remove` it, save the list, and show `Can't find {file name}` as the
  status bar error. Otherwise open it the way a dropped file opens
  (`PendingAction::Open(Some(path))`, which asks Save/Discard/Cancel first when the
  document is modified).
- `Effect::Exit`: the same as the window's close button: when the document is modified
  the unsaved-changes question comes first; otherwise the app closes
  (`ViewportCommand::Close` goes through the existing close handling).
- Tests that open the real window set `MODELER_CONFIG_DIR` to a temporary folder so they
  never touch the user's list.

## Tests

`tests/menus.rs` (egui_kittest: File, Tools and Help menus, `recent_labels`,
`about_lines`) and `tests/recent.rs` (the `recent` module). Existing chrome tests must
keep passing.
