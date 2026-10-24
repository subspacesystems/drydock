# m4-ui-axes: red, green and blue axes at the origin

Milestone M4, from Tyrel's UI list (item 2).

## Problem

The viewport shows no axes, so there is no way to see where the origin is or which way
red (x), green (y) and blue (z) run. Inference already colours axis-locked lines red,
green and blue; the axes themselves should be visible too.

## What to build

### `overlay::axes` (`src/overlay.rs`)

```rust
pub fn axes(transform: DMat4, half_length: f64) -> Vec<Primitive>
```

The three axes of the frame `transform` maps from (local to world), as six
`Primitive::Line`s in this order: red positive, red negative, green positive, green
negative, blue positive, blue negative. Axis `i` is local x, y, z for `i` = 0, 1, 2.

- Every line starts (`a`) at the frame's origin, `transform.transform_point3(DVec3::ZERO)`.
- Its end (`b`) is `origin + dir * half_length` for the positive half and
  `origin - dir * half_length` for the negative half, where `dir` is
  `transform.transform_vector3(unit axis)` normalised. A scaled transform therefore still
  gives lines `half_length` long in world units.
- Style: `Style::Axis { axis: i, positive: true }` for the positive half, `positive: false`
  for the negative half.
- An axis whose mapped direction is zero or not finite (for example a transform that
  scales z to 0) is left out (both its halves); the others stay, in order.
- `half_length` that is not finite or not greater than 0, or an origin that is not
  finite, gives an empty list.

### `overlay::clip_in_front`

```rust
pub fn clip_in_front(camera: &Camera, a: DVec3, b: DVec3) -> Option<(DVec3, DVec3)>
```

The window projects line ends one by one, and a point behind a perspective camera has no
pixel, so today a long line with one end behind the camera disappears entirely. This
function keeps the part of the segment that can be drawn.

- Depth of a point `p` is `(p - camera.eye()).dot(forward)` with `forward` from
  `camera.basis()`. The near distance is `camera.distance / 1000.0`.
- Parallel projection: return `Some((a, b))` unchanged (finite input).
- Perspective: an end with depth `>= near` is kept as it is. If both ends are nearer
  than `near` (or behind), return `None`. If exactly one is, move that end along the
  segment to the point whose depth is exactly `near` (linear interpolation). The ends keep
  their order: the result's first point comes from `a`, the second from `b`.
- Any non-finite coordinate in `a` or `b` gives `None`.

### View state and menu

- `Action::ToggleAxes` (`src/input.rs`, on main). `ViewState::axes` (on main) starts
  `true`; `ViewState::apply(Action::ToggleAxes, _)` flips it and returns `Effect::None`
  (the camera is untouched).
- `MenuState::axes` (on main) says whether the axes are shown. The View menu gets an
  item labelled exactly `Axes`, after the Hide/Show Rest of Model item (separator
  before it is fine), drawn checked when `state.axes` is true (`ui.checkbox` or
  `ui.selectable_label` are both fine). Clicking it returns `Action::ToggleAxes` and
  closes the menu. It is there whether or not a model is open.

### Window (`src/app.rs`; not covered by tests, checked in the real window at review)

- `overlay_style` gets colours for `Style::Axis`: red, green and blue that read on the
  light viewport background (for example (200, 40, 40), (40, 150, 40), (40, 70, 210)),
  1.5 px wide.
- `paint_primitives` draws `Style::Axis { positive: false, .. }` lines dashed
  (`egui::Shape::dashed_line`, about 6 px dash and 4 px gap) and the positive halves solid.
- Every `Primitive::Line` (all styles, so guides benefit too) goes through
  `overlay::clip_in_front(&self.view.camera, a, b)` before its ends are projected; lines
  it drops are not drawn. `paint_primitives` takes the camera for this.
- `paint_overlays`: when `self.view.axes`, draw `overlay::axes(open_transform, half)`
  first (under the selection and tool overlays), with
  `half = self.view.camera.distance * 100.0`. `open_transform` is the open instance's
  placement, as already computed there (identity at the top level), so while editing
  inside an instance the axes sit at that instance's origin and follow its rotation;
  the model's own axes are not drawn then. With an empty document the axes still show.
- `MenuState` gets `axes: self.view.axes` (already on main).

## Tests

`app/crates/modeler/tests/axes.rs` (spec commit; don't edit). Run with
`cargo test -p modeler --test axes`.
