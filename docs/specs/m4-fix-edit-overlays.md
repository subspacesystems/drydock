# m4-fix-edit-overlays: selection and tool overlays inside an open instance

Milestone M4, follow-up from Tyrel's M4 review.

## Problem

After a double click opens a component instance for editing, clicking edges or faces
inside it selects them (other panels react), but nothing is highlighted in the viewport.
The window's overlay code (`App::paint_overlays` in `src/app.rs`) always looks the
selection up in the root body and draws its points as they are, while the editor's
selection, Push/Pull face and Scale box live in the open body, whose coordinates differ
from the world by the open instance's placement.

## What to build

### `overlay::selection` (`src/overlay.rs`)

```rust
pub fn selection(model: &Model, body: BodyId, transform: DMat4, selection: &[Entity]) -> Vec<Primitive>
```

The highlight for `selection`, whose entities belong to `body`. Every point is mapped
through `transform` with `DMat4::transform_point3` (body to world).

- `Entity::Edge`: one `Primitive::Line` between the edge's two vertices, `Style::Selected`.
- `Entity::Face`: the face as triangles, each a `Primitive::Polygon` with exactly three
  points and `Style::SelectedFace`. The triangles cover the face exactly once: holes stay
  uncovered, and their total area equals the face's area (times the transform's area
  scale). Use the kernel's triangulation (`modeler_kernel::tess::triangulate` on the
  loops projected into the face plane with `geom::to_plane_2d`, outer loop first, then the
  holes' points in order). A face drawn as one polygon is wrong: non-convex faces and
  faces with holes paint over their holes.
- `Entity::Instance`: one `Style::Selected` line per segment of
  `modeler_kernel::select::instance_segments(model, body, id)`, mapped.
- Entities that don't exist in `body`, or a `body` that doesn't exist, produce nothing.
  Order: the selection's order, each entity's primitives together.

### `overlay::transformed`

```rust
pub fn transformed(primitives: Vec<Primitive>, transform: DMat4) -> Vec<Primitive>
```

The same primitives, in the same order and styles, with every point (line ends, polygon
points, marker positions) mapped through `transform.transform_point3`.

### Styles

Two new `Style` variants (already on main): `Selected` (a selected edge or instance edge)
and `SelectedFace` (a triangle of a selected face). In `src/app.rs`, `paint_primitives`
draws `SelectedFace` polygons as a fill only, no outline, with the accent color at alpha
55 (the tint the root selection uses today), and `Selected` lines 3 px wide in the
accent color. Keep `overlay_style` exhaustive.

### Window wiring (`src/app.rs`, `paint_overlays`)

Replace the hand-written root-only selection drawing with `overlay::selection`, using the
editor's open body and its world transform: `editor.path().body(model)` (root when it
doesn't resolve) and `editor.path().transform(model)` (identity when it doesn't resolve).
In the same function, draw these through `overlay::transformed` with that transform too,
because they are computed in the open body's coordinates:

- the Push/Pull face highlight (`overlay::face_highlight` on the open body's geometry, not
  `kernel.root()`; the face id belongs to the open body),
- the Scale tool's box and handles (`scale::bounds(kernel, <open body>, selection)` when
  the tool has no preview).

Guides stay as they are (they are root guides in world coordinates). Previews, hints and
the protractor already arrive in world coordinates from the editor; don't transform them.

At the root path the result must look exactly like today: identity transform, root body.

## Tests

`crates/modeler/tests/edit_overlays.rs` (12 tests): edges, faces with and without holes,
nested instances, rotation/scale/mirror transforms, missing entities, `transformed`.
The window wiring has no automated test; the orchestrator checks it in the real window.

## Out of scope

Highlight colors for other states, hover highlights, the faded-rest drawing itself.
