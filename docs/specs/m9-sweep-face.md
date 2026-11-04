# m9-sweep-face: sweep a profile face along a path (kernel)

Implement `app/crates/drydock-kernel/src/sweepface.rs` (skeleton on main: `MIN_PROFILE_ANGLE_DEG`,
`MAX_TURN_DEG`, `SweepError`, `Swept`, `sweep_sections`, `sweep_face`). Change only that file.
Acceptance tests: `app/crates/drydock-kernel/tests/sweep_face.rs`, `tests/sweep_face_props.rs`
(fixtures in `tests/sweep_face_common/` and the push/pull ones in `tests/pushpull_common/`).

Follow Me sweeps a flat profile face along a path of edges: a round profile along a bent path
makes a pipe, a moulding profile round a face's outline makes a frame. This task is the
geometry. The path type is `sweep::SweepPath` (`docs/specs/m9-sweep-path.md`); the tool that
picks the profile and path comes later (`m9-followme-tool`).

**Use only `path.points`, `path.closed` and, to name a vertex in an error, `path.vertices`.**
The path's vertices and edges need not be in the body (the tests mostly pass paths made of
bare points with made-up ids). Don't call `SweepPath::reversed`, `SweepPath::length`,
`path_from_edges` or `face_perimeter`: they belong to another task and may not be written
yet. Reverse or rotate the points yourself.

## Terms

- `n`: the profile face's unit front normal (`Context::face_plane`). **Centroid**: the mean of
  the positions of the profile's outer-loop vertices.
- Path points `p[0..k]`. **Segment** `i` runs from `p[i]` to `p[i+1]` (closed: the last one
  from `p[k-1]` back to `p[0]`), with unit direction `d[i]`.
- **Turn** at a vertex: the angle between the directions of the segments arriving and
  leaving.
- **Mitre plane** at a joint `p[i]` (a vertex with a segment on each side): through `p[i]`,
  normal `normalize(d_in + d_out)`.
- **Carry** a point `x` along `d` onto plane `(q, m)`: `x + d * ((q - x)·m / (d·m))`. The
  point's **travel** is `(carried - x)·d`.
- A **section** is the profile's loops carried onto one plane: outer loop first, then each
  hole in the face's order; each loop's points in the profile loop's own order (from each
  use's start vertex, `Context::loop_points`).

## Checks, in this order

Every refusal leaves the model and the undo stack exactly as they were. `sweep_sections`
makes the same checks (from `MissingFace` on, up to and including `Twisted`) and changes
nothing.

1. `MissingBody(body)`, then `MissingFace(profile)`.
2. `BadPath`: fewer than 2 points (open) or 3 (closed); `vertices.len() != points.len()`; a
   point that is not finite; a segment (closed: the closing one too) no longer than
   `LINEAR_TOL`.
3. The profile has no plane: `Invalid(Violation::NonPlanarFace(profile))`.
4. `ProfileShared(e)`: some edge of the profile (any loop) is also used by another face; `e`
   is the lowest such edge id. The profile must be a free face.
5. `DegenerateMitre(v)`: at each joint (open: `p[1]` to `p[k-1]`; closed: every vertex, in
   index order), a turn over `MAX_TURN_DEG` (179 degrees); `v` is `path.vertices` of the
   first such joint.
6. Where the sweep starts (next sections).
7. `ProfileParallel`: `|n · d| < sin(MIN_PROFILE_ANGLE_DEG)` (1 degree), where `d` is the
   direction the sweep starts along (below).
8. `Folded(v)`: while building the sections in sweep order, some point's travel is
   `LINEAR_TOL` or less; `v` is the path vertex of the plane it was carried onto. (The
   profile is too big for a turn, or lies beyond the first joint.)
9. `Twisted` (closed paths, below).

After building, as push/pull does: a `validate` violation that wasn't there before gives
`Invalid(v)` (the first new one); a new pair from `face_overlaps` gives `Overlap(a, b)`. Roll
the edit back first.

## Open paths

- **Start end.** The sweep starts at the end nearer the centroid: it runs the path reversed
  when `|p[last] - centroid| < |p[0] - centroid| - LINEAR_TOL`, otherwise as given. From here
  on `p` and `d` are in sweep order, and vertex ids in errors follow that order too.
- The sweep starts along `d[0]`.
- **Sections.** Section 0 is the profile itself, where it is (its plane need not pass
  through `p[0]`; the sweep runs from the profile's plane). Section `i` (1 to `k-1`) is
  section `i-1` carried along `d[i-1]` onto the mitre plane at `p[i]`. The last section is
  carried along the last segment's direction onto the **end plane**: through the path's last
  point, normal to the last segment. So the end cap is square to the path, whatever the
  profile's slant.
- **Faces.**
  - The profile stays as the **start cap** (same id), fronting away from the sweep: if
    `n · d[0] > 0`, turn it (reverse every loop, same id, by removing and re-adding the
    face). Its `FaceMaterials` stay as they were.
  - One **side** face per profile edge per segment (holes too).
  - The **end cap**: a new face on the last section (with holes), fronting along the last
    segment. It gets the profile's front and back material ids (no texture maps); nothing
    is set when the profile has neither.
- `Swept { start: Some(profile), end: Some(cap), sides }`.

## Closed paths

The profile sits somewhere on the ring; the sweep goes once round and closes on itself.

- **Where the profile meets the path.** Let `s(x)` be the signed distance of `x` from the
  profile's plane. Candidates, in path order:
  - each vertex `p[j]` with `|s(p[j])| <= LINEAR_TOL` (order position `j`);
  - each segment `i` whose ends both have `|s| > LINEAR_TOL` with opposite signs, at the point
    where it crosses the plane (order position `i + 0.5`).

  None: `NotOnPath`. Otherwise take the candidate nearest the centroid; a later one wins
  only when nearer by more than `LINEAR_TOL`.
- **At a vertex `p[j]`:** every profile point must lie within `LINEAR_TOL` of the mitre plane
  at `p[j]`, else `MitreMismatch(vertices[j])`. The sweep starts along `d[j]`. Section 0 is
  the profile itself, at `p[j]`.
- **Part way along segment `i`:** the sweep starts along `d[i]`. Section 0 is the profile
  carried along `d[i]` onto the mitre plane at the segment's far end, `p[i+1]`.
- **Sections.** Following the path's own direction from there, each section is the
  previous one carried along the segment between them onto the next vertex's mitre plane:
  one section per path vertex. Then carry the last section once more, along the segment
  back to section 0's vertex. Each of these carries is checked for `Folded` (the vertex is
  the one whose mitre plane the points were carried onto). Any carried point further than
  `LINEAR_TOL` from its own point in section 0 gives `Twisted`. (A flat ring always closes;
  a path that isn't flat turns the profile on the way round.)
- **Faces.** Side faces only, round the ring, from each section to the next and from the
  last back to section 0. The profile face goes (set its materials to the default first, so
  undo brings them back); then each of the profile's edges that no face uses any more goes,
  and then each of its vertices that no edge uses. At a vertex start the profile's own
  vertices and edges are section 0, so they stay.
- `Swept { start: None, end: None, sides }`.

## Building

- Every section point that is within `LINEAR_TOL` of an existing vertex (`find_vertex`) uses
  that vertex; otherwise insert a new one. Every edge the faces need between two vertices
  that already have an edge (`edge_between`) uses that edge, in the right direction;
  otherwise insert a new one. So a profile standing on its path shares the path's vertices
  and edges rather than doubling them, and a closed sweep from a vertex reuses the profile.
  The path itself is never changed.
- **Orientation.** Take the profile's loops in the order that fronts along the start
  direction (`n · d >= 0`: as they are; otherwise each reversed). For each such loop use
  from `a` to `b`, and each step from section `i` to section `j`, the side face's outer loop
  is `a_i -> b_i -> b_j -> a_j -> back to a_i` (the same pattern as push/pull's sides). Then
  every face of the result fronts outward, holes included.
- **Merging.** Candidates: the edges the sweep inserted, plus (closed paths) the profile's
  edges still there. Going through them in ascending id order: if an edge still exists and
  exactly two faces use it, both side faces of this sweep, in the same plane
  (`faces::same_plane`) and fronting the same way, erase it with `Context::erase_edge`,
  which merges the two (the lower id stays). Then erase each candidate no face uses any
  more. This happens where the path runs straight through a vertex, where the profile has
  collinear edges, and wherever a flat path keeps a side in one plane (the top of an L, the
  top and bottom of a flat ring, which become faces with holes).
- `sides`: the side faces still there afterwards, ascending.
- Do all of it in one `ModelEdit` and push it as one undo step labelled `label`.

## Tests

- `sweep_face.rs`: refusals in check order (body, face, bad paths, shared profile, a path
  doubling back, parallel profile, folding at a tight turn, beyond the first joint and in a
  tight ring, a profile off a closed path, at a corner off its mitre, a ring that isn't flat,
  overlap with an existing face, crossing an existing edge), each leaving the model
  unchanged; open paths (straight box, profile turned to front backwards, end cap materials,
  the nearer end starts, a profile beyond the path end, L-path sections and merged top and
  bottom, slanted profile with a square end, a tube from a profile with a hole, merges across
  a straight joint and along collinear profile edges, a profile curve kept on the start cap,
  one undo step); closed paths (from a profile at a corner, part way along a side, the
  sections' positions, either direction gives the same faces, materials dropped and restored
  by undo, a moulding round a face reusing the face's vertices and edges).
- `sweep_face_props.rs`: a straight sweep square to the profile equals push/pull of the same
  face (polygons with and without holes, any orientation and direction); a rectangle round
  an L has volume `w h (a + b - 2 e tan(turn/2))` and is solid; a rectangle round a flat
  convex ring has volume `w h (perimeter + sum of 2 e tan(turn/2))` with `2n + 2` sides; a
  small square centred on any open path (not flat) has volume area times length; reversing
  the path never changes the faces.

## Not in scope

Cutting into neighbouring faces (the profile must be free; overlaps are refused); a sweep
that runs into itself away from a joint (only folding at a joint is caught); arcs on the
path or profile becoming smooth curved surfaces, or curves on the new edges
(`m9-sweep-curves`); the tool and preview drawing (`m9-followme-tool`); merging collinear
edges or removing the vertices left on straight runs.

## Where to look

- `pushpull::push_pull_into`: the closest existing operation (building, `roll_back`, the
  before/after `validate` and `face_overlaps` checks, materials); `PushPullError`
- `sweep::SweepPath`
- `Context::face_plane`, `loop_points`, `loop_vertices`, `faces_of_edge`, `edges_of_vertex`,
  `find_vertex`, `edge_between`, `insert_vertex`, `insert_edge`, `insert_face`,
  `delete_face`, `delete_edge`, `delete_vertex`, `apply`, `erase_edge`, `validate`,
  `face_overlaps`
- `faces::same_plane`, `geom::Plane`, `geom::LINEAR_TOL`
- `Model::edit_geometry`, `Model::set_face_materials`, `Model::apply_edit`, `ModelEdit`,
  `UndoStack::push`, `Body::face_materials`, `FaceMaterials`
- `EdgeUse::forward`, `EdgeUse::backward`, `Loop`, `Face`, `Record::RemoveFace`,
  `Record::AddFace`
- Test helpers: `path`, `reversed`, `sweep`, `same_faces`, `rect_points`, `ngon`, `basis`
  (`sweep_face_common`); `mesh`, `cube`, `face_points`, `normals_outward` (`pushpull_common`);
  `square_x`, `rect_x`, `square_ring`, `corner_profile` (`sweep_face.rs`)
