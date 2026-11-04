# m9-offset-face: offset a face's loops in its plane

Milestone M9 (Offset). Kernel only; the Offset tool (typed distance, preview) is a later task
that calls these two functions.

Fill in `app/crates/drydock-kernel/src/faceoffset.rs` (skeleton on main: the types, docs and
two `unimplemented!()` functions). Don't change the public types or signatures. Acceptance
tests: `tests/face_offset.rs` and `tests/face_offset_props.rs` in `drydock-kernel`.

This is not `offset.rs` (the M7 clearance offset, which moves a solid's faces along their
normals). Leave that file alone.

## What it does

Every boundary loop of the face moves sideways in the face's plane by the same distance, so
the face's region grows (`distance > 0`, "outward") or shrinks (`distance < 0`, "inward"). The
outer loop grows outward; holes move the other way (an outward offset shrinks the holes, an
inward one grows them), because each loop moves away from the face's material.

The result is drawn as new edges and faces:

- **Inward**: the face splits. The original face keeps its id and its old outer loop and
  becomes the **ring**: its only hole is the new (offset) outer loop. A new **inner** face is
  bounded by the new outer loop, with the new hole loops as its holes. For each old hole, a
  new **hole ring** lies between the new hole loop (its outer loop) and the old hole loop (its
  hole). The old hole loops are no longer holes of the original face.
- **Outward**: the original face is not touched. A new **ring** has the new outer loop as its
  outer loop and the old outer loop as its only hole. For each old hole, a new **hole ring**
  lies inside the old hole: the old hole loop is its outer loop, the new (smaller) hole loop
  its hole.

Rule to remember: the original face keeps its id and its old outer edges; everything else is
new. `FaceOffset` reports the faces (`inner`, `ring`, `hole_rings` in hole order) and the new
edges of each loop (`edges`).

## Geometry of one loop

Let `n` be the face's front normal (`Context::face_plane(face)`, unit length). Take the old
loop's points `p[0..k]` in loop order (`Context::loop_points`; the outer loop runs
counter-clockwise about `n`, holes clockwise, so the material is always on the left). For an
edge with unit direction `e`, its outward side is `e × n` (right of the edge, away from the
material). This one formula covers holes too.

For each vertex `p[j]`, with incoming unit direction `e0 = normalize(p[j] - p[j-1])` and
outgoing `e1 = normalize(p[j+1] - p[j])` (indices wrap), `n0 = e0 × n`, `n1 = e1 × n`,
`c = n0 · n1`:

- If `e0` or `e1` is zero, or `c < -1 + 1e-9` (the loop doubles back), refuse with
  `LoopsCross`. (A valid face can't have either.)
- Mitre vector `m = (n0 + n1) / (1 + c)`; `turn = (e0 × e1) · n` (positive at a corner that is
  convex for the material).
- **Bevel** when `distance * turn > 0` (the two offset edges move apart at this corner) and
  `|m| > MITRE_LIMIT` (4.0; that's an angle under about 29 degrees). The corner gives two
  points, in order: `p[j] + distance * n0`, then `p[j] + distance * n1`.
- Otherwise the corner gives one **mitre** point `p[j] + distance * m`. Collinear vertices
  stay as a vertex (`m = n0`).

The new loop's points are the corners' points in loop order, starting with vertex 0's. Each
new edge between corner `j` and corner `j+1` is parallel to the old edge `p[j]..p[j+1]` and
`|distance|` from it.

## Checks, in this order (both functions)

1. `MissingBody(body)` (only `offset_face`), then `MissingFace(face)`.
2. `BadDistance`: not finite, or `|distance| <= LINEAR_TOL`.
3. `NotPlanar(face)`: `face_plane` is `None`, or any point of any of its loops is farther than
   `LINEAR_TOL` from that plane (`geom::max_plane_deviation`).
4. Compute every loop's corners (doubling back gives `LoopsCross`, see above).
5. `Collapsed`, over all loops (outer first, then holes, edge by edge): for old edge `j` with
   unit direction `e`, the new edge runs from the **last** point of corner `j` to the **first**
   point of corner `j+1`; if `(to - from) · e <= LINEAR_TOL`, the edge vanished or turned
   around: refuse. (An inward offset too large for the outer loop, an outward one too large
   for a hole, or a short edge between two reflex corners.)
6. `LoopsCross`, with the new loops' points in 2-D (`geom::to_plane_2d(points, n)`):
   - each new loop must pass `geom::polygon_is_simple_2d(poly, LINEAR_TOL)`;
   - no edge of one new loop may meet any edge of another new loop
     (`geom::segment_intersection(.., LINEAR_TOL)` is not `SegmentHit::None`);
   - every point of each new hole loop must be `Containment::Inside` the new outer loop
     (`geom::point_in_polygon_2d(.., LINEAR_TOL)`).

`offset_loops` stops here and returns the new loops' points: outer first, then holes in the
face's order, each as described above. It never changes anything.

`offset_face` then builds the result in one `Model::edit_geometry` call:

7. Build the edges and faces (next section). Then copy the original face's `FaceMaterials`
   (front, back and both texture maps) onto every face in the result whose materials differ
   from it (the inward ring was removed and re-added, so check it too). Unpainted stays
   unpainted.
8. `Invalid(v)`: the first violation of `Context::validate()` after the edit that wasn't in
   the list before it.
9. `Overlap(a, b)`: the first pair of `Context::face_overlaps()` after the edit that wasn't in
   the list before it.

On any refusal the model and the undo stack are exactly as before (roll the edit back with its
inverse, as `push_pull` does). On success, push the edit as one undo step labelled `label`.
Undo restores the model exactly; redo gives the same model again.

No edge splitting or sticky drawing: the new vertices and edges are inserted as they are with
`Context::insert_vertex` / `insert_edge` (not `add_edge`). A new edge that crosses or touches
existing geometry shows up as a new `validate` violation in step 8 and is refused. That
includes outward offsets whose hole ring lands on a face filling the hole (a window in a
wall): the hole ring's outer loop is that face's outer loop, so it's a `DuplicateFace`. That's
a known limit for now.

## Building the faces

For each loop (outer first, then holes): insert one vertex per corner point, in loop order;
insert edge `i` from vertex `i` to vertex `i+1` (the last back to the first); the new loop uses
them all forward, so it runs the same way as the old loop. `edges[loop]` lists them in that
order.

`reverse(L)` below is the loop with its uses in reverse order and each use's `reversed`
flipped.

- **Inward** (`distance < 0`): remove the original face, then re-add it under the **same id**
  (push `Record::AddFace(face, ..)` through `Context::apply` and append it to the log, the way
  `repair.rs` turns a face): outer = old outer loop, inner = `[reverse(new outer)]`. Then
  insert the inner face: outer = new outer, inner = the new hole loops. Then one hole ring per
  hole, in order: outer = `reverse(new hole)`, inner = `[old hole]`.
- **Outward**: insert the ring: outer = new outer, inner = `[reverse(old outer)]`. Then one
  hole ring per hole: outer = `reverse(old hole)`, inner = `[new hole]`.

Every face then fronts the same way as the original.

## Curves

Curves (`curve.rs`) whose `curve::curve_edges` all lie on the face's loops are carried to the
new loop where the offset keeps them concentric; elsewhere the new loop is plain segments.
Map each old chain vertex to its new vertex (only one-point corners map; a bevelled corner
maps to nothing). Then, inside the same `edit_geometry` call, after the faces:

1. If every vertex of the chain maps, `curve::fit(ctx, &mapped_chain, old.start)`. Accept it
   if its center is within `LINEAR_TOL` of the old curve's center and every chain vertex is
   within `LINEAR_TOL` of the circle (distance to the center differs from the radius by at
   most `LINEAR_TOL`, and off the curve's plane by at most `LINEAR_TOL`).
2. Otherwise, for an **open** curve with at least 5 chain vertices: the same with the chain's
   first and last vertex left out. (An arc's two end vertices are mitred with the edges next
   to the arc and leave the circle; the vertices between them stay on it.)
3. Otherwise no curve.

Insert an accepted curve with `Context::insert_curve`. The old curves stay on the old edges,
unchanged. Why the center check: `fit` fits a circle through three of the points, so four
points of an isosceles trapezoid always "fit", just not concentrically.

## Tests

- `tests/face_offset.rs`: loop points (squares, L-shape, collinear vertex, holes, back-facing
  and tilted faces, bevel and mitre limit both ways, notch), each refusal and that it leaves
  the model and undo stack alone, the faces made inward and outward with and without holes,
  materials, a box top inward then push/pull, outward flange on a box top, inside a group,
  curves (circle, circular hole, arc meeting a line, short arc dropped, 3-step circle).
- `tests/face_offset_props.rs`: for random convex polygons in tilted planes, offset then
  offset back by `-d` restores the polygon; new edges are parallel at distance `d`; area grows
  with the distance; `offset_face` splits the area correctly and undo/redo are exact. For
  random star-shaped polygons, area grows with the distance whenever both offsets succeed,
  and a refusal changes nothing.

Run: `harness/bin/run-tests -p drydock-kernel`.

## Where to look

- `faceoffset.rs` skeleton: `FaceOffset`, `FaceOffsetError`, `MITRE_LIMIT`, `offset_loops`,
  `offset_face`.
- How push/pull builds faces and rolls back: `push_pull`, `push_pull_into` (its
  `reverse_loop`, `roll_back`, and the before/after `validate` / `face_overlaps` diff).
- Re-adding a face under its id: `turn_face` and its caller in `repair.rs`
  (`Record::AddFace`, `Context::apply`).
- Topology: `Context::face`, `Context::face_plane`, `Context::loop_points`,
  `Context::loop_vertices`, `Context::insert_vertex`, `Context::insert_edge`,
  `Context::insert_face`, `Context::delete_face`, `Context::insert_curve`, `Context::apply`,
  `Context::validate`, `Context::face_overlaps`, `Loop`, `EdgeUse`, `Face`, `Record`.
- Model: `Model::edit_geometry`, `Model::apply_edit`, `Model::set_face_materials`,
  `Body::face_materials`, `ModelEdit::inverse`, `UndoStack::push`.
- Geometry: `geom::max_plane_deviation`, `geom::to_plane_2d`, `geom::polygon_is_simple_2d`,
  `geom::segment_intersection`, `SegmentHit`, `geom::point_in_polygon_2d`, `Containment`,
  `LINEAR_TOL`.
- Curves: `curve::curve_edges`, `curve::fit`, `Curve`, `Curve::is_closed`.
