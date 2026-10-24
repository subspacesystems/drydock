# m4-ui-status: a lean status bar, Model Info, measurement in the status bar

Milestone M4, from Tyrel's UI list (items 3, 4 and 11).

## Problem

The status bar carries triangle, line, material and face/edge counts that mean little
while drawing, and the measurement box floats over the viewport's bottom right corner,
hiding the model. Counts belong in a window opened on request; the measurement belongs
in the status bar.

## What to build

### `chrome::model_info_lines`

```rust
pub fn model_info_lines(stats: &Stats, kernel: bool) -> Vec<String>
```

Lines top to bottom: `Triangles: N`, `Lines: N`, `Materials: N`,
`Size: <format_size(stats.size_m)>`, then, only when `kernel` is true, `Faces: N` and
`Edges: N`.

### `chrome::model_info_window`

```rust
pub fn model_info_window(ctx: &egui::Context, open: &mut bool, lines: &[String])
```

Like `about_window`: draws nothing while `*open` is false; otherwise a non-collapsible,
non-resizable window titled `Model Info` with one label per line and a `Close` button
that sets `*open` false (the title bar's close does too).

### `chrome::status_measurement`

```rust
pub fn status_measurement(ui: &mut egui::Ui, measurement: Option<(&str, &str)>, typed: &str)
```

Draws into the status bar's `ui` (a horizontal layout): when `measurement` is
`Some((label, value))`, a label with `label` and a monospace label with `value`
(omit the value label when it is empty); then, when `typed` is not empty, a monospace
label with `typed` (also when `measurement` is `None`). With nothing to show it draws
nothing.

### Input and menu

- `Action::ModelInfo` (skeleton on main) → `Effect::ModelInfo` from `ViewState::apply`,
  camera untouched.
- View menu: a `Model Info` button (after Axes, behind a separator) choosing
  `Action::ModelInfo`, and closing the menu.

### App (`src/app.rs`, untested, orchestrator checks it in the real window)

- `Effect::ModelInfo` opens the window (`model_info_open` field next to `about_open`);
  it shows `model_info_lines` of the open model's stats (`kernel` = the document has a
  kernel model); with no model it shows nothing and the flag is cleared.
- The status bar no longer shows triangle/line/material/size or face/edge counts. It
  keeps the file name, the editor message (error colour), kernel error, Saved, warnings
  and error, and on the right the projection and tool (and "Ctrl: copy").
- The floating `measurement_box` is removed. The right side of the status bar, before
  the projection, shows `status_measurement` with the editor's `measurement()` (label
  `Distance (copy)` for Move with copy on, as today) and `typed()`.
- Unused-definition counting stays as is.

## Tests

`tests/status.rs` (kittest and pure). Do not change it or other test files except where
a test asserts the removed status text (none known).
