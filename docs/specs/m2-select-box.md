# m2-select-box: box selection and connected selection

Milestone M2. Crate `app/crates/modeler-kernel`, file `src/select.rs` only. Replace the `unimplemented!("m2-select-box")` bodies; don't change public signatures or types. Private helpers are fine; `crate::geom` has segment and polygon helpers you may use. No new dependencies. Tests: `tests/select.rs`, `tests/select_props.rs` (fixed; don't edit). Library code never panics, whatever the input.

Every function returns its entities sorted by `Entity`'s `Ord` (edges, then faces, then instances, each by id) with no duplicates.

## `BoxMode::from_drag(start, end)`
`Window` when `end.x >= start.x` (dragged left to right, or straight up or down), `Crossing` when `end.x < start.x`. Only `x` matters. If either `x` is NaN, `Window`.

## `box_select(model, body, project, a, b, mode)`
The box is the axis-aligned screen rectangle with opposite corners `a` and `b`, in any order, boundary included. A zero-width or zero-height box is allowed. If a coordinate of `a` or `b` is not finite, or `body` doesn't exist, the result is empty.

Candidates are the entities directly in `body`: its edges, its faces and its instances. Geometry inside an instance is never returned on its own; it only decides whether the instance is selected.

**Points and shapes of an entity**, in `body` coordinates:
- Edge: the segment between its two vertices.
- Face: every vertex of its outer loop and its holes; boundary segments are the edges of all its loops; the area is inside the outer loop and outside every hole.
- Instance: every edge and face of its definition's body, moved by the instance's `transform`, plus, recursively, those of the instances in that body (transforms compose: the outer instance's transform applied after the inner one's). An instance whose definition has no vertices anywhere (recursively) is never selected.

**Visibility.** A point is seen when `project` returns `Some` with finite coordinates. An entity with any point that isn't seen is never selected, in either mode. (For an instance this means every vertex of all its geometry.)

**Window**: an entity is selected when all its projected points lie in the box.

**Crossing**: an entity is selected when it touches the box:
- Edge: its projected segment and the box share a point (an end inside the box, or the segment passing through it).
- Face: one of its projected boundary segments touches the box, or the center of the box lies inside the projected area (inside the outer loop, outside every hole). The second rule catches a box drawn wholly inside a face.
- Instance: any of its edges or faces (as above, recursively) touches the box.

The box ignores what's in front: hidden entities behind others are selected too. Consequences the tests check: anything `Window` selects, `Crossing` selects with the same box; swapping `a` and `b` changes nothing; a bigger box never selects less.

## `bounded(ctx, entity)` (double click)
- Edge: the edge plus every face that uses it.
- Face: the face plus every edge of its outer loop and its holes.
- Instance: just the instance (`ctx` is not consulted).
- An edge or face that isn't in `ctx`: empty.

## `connected(ctx, entity)` (triple click)
Every edge and face connected to the entity. Two edges are connected when they share a vertex; a face is connected to the edges of its loops. Start from the edge itself, or from a face's edges, and take everything reachable. A lone edge gives itself.
- Instance: just the instance.
- An edge or face that isn't in `ctx`: empty.
