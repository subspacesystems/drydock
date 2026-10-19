# m1-sticky-edges: drawing edges that join existing geometry

Milestone M1, kernel task. Crate `app/crates/modeler-kernel`, file `src/sticky.rs`. Implement every `unimplemented!("m1-sticky-edges")`. Don't change public signatures or types; add private helpers freely (in `sticky.rs`). `src/geom.rs` and `src/topology.rs` are done: use them, don't edit them. Dependencies: `glam` only. No panics on any input.

Design background: `docs/adr/004-kernel-design.md`. Geometry in one `Context` is "sticky": a new edge never just lies across or along an old one. Where they cross, touch or overlap, both are split at a shared vertex, and a stretch they share is one edge.

## Ground rules
- Change the context only through the `topology` primitives (`insert_*`, `delete_*`) and, for replacing a face under the same id, `Context::apply` of a `RemoveFace` and then an `AddFace` record with that id. Push every record you apply onto `log`, in the order applied. Records already in `log` stay; you only append.
- On any error the context and `log` are exactly as before the call (roll back with the inverses of what you applied, then truncate `log`).
- All tolerances are `geom::LINEAR_TOL`. "Within tolerance" means distance `<=` it.
- The guarantees below hold when the context starts valid (`validate()` empty). On an invalid context, don't panic; the result is unspecified.
- Existing vertices never move. No faces are created by these operations (face creation is the next task, `m1-sticky-faces`).

## split_edge(e, p, log)
- `MissingEdge(e)` if absent; `NotFinite` if `p` has a NaN or infinite coordinate.
- Let `q` be the point on the segment `e` closest to `p`. If `q` is within tolerance of an end of `e`, return that end (the closer one; `ends[0]` on a tie) and record nothing.
- Otherwise: `insert_vertex(q)` = `m`; `insert_edge(ends[0], m)` = piece 1; `insert_edge(m, ends[1])` = piece 2; then for each face using `e` (ascending id) replace it under the same id with every use of `e` swapped for the two pieces: a forward use becomes `forward(piece 1), forward(piece 2)`, a reversed use becomes `backward(piece 2), backward(piece 1)`, at the same place in the loop. Then `delete_edge(e)`. Return `m`.

## add_edge(a, b, log)
1. `NotFinite` if any coordinate of `a` or `b` is NaN or infinite. `Degenerate` if `a` and `b` are within tolerance.
2. Snap each end, `a` first, then `b`:
   - an existing vertex within tolerance (`find_vertex`) is the end;
   - else the edge closest to the point, if within tolerance (`geom::point_segment_distance`; lowest id on a tie), is split there with `split_edge`, and the vertex it returns is the end;
   - else `insert_vertex(point)`.
   If both ends come out as the same vertex: `Degenerate` (roll back).
3. Let `A`, `B` be the positions of the two end vertices. Test the segment `A`-`B` against every edge with `geom::segment_intersection(A, B, P0, P1, LINEAR_TOL)` (`P0`, `P1` that edge's end positions):
   - `Point { tb }` with `tb` strictly between 0 and 1: the segment crosses or touches the edge inside it. Split that edge at its point at `tb` (so the new vertex lies on the old edge).
   - `Point` with `tb` 0 or 1: that end vertex of the edge lies on the segment.
   - `Overlap`: the segment runs along the edge; both end vertices of the overlapped stretch lie on the segment (after step 2, they are existing vertices).
   Collect every vertex found on the segment this way, plus the two ends. If two collected points are within tolerance of each other, they are one vertex.
4. Sort them by their parameter along `A`-`B`. Join each consecutive pair with the existing edge between them (`edge_between`) if there is one, else `insert_edge(earlier, later)`. The result lists the vertices from the `a` end to the `b` end and the edges between them.
5. If the segment is already fully covered by existing edges between existing vertices, nothing is recorded and the chain lists those edges.

Faces never change shape: a face using an edge that gets split keeps its id and gets both pieces (as in `split_edge`). An edge that passes through a face without touching its edges leaves it alone.

## erase_edge(e, log)
- `MissingEdge(e)` if absent.
- For each face using `e`, ascending id: if `e` is in its outer loop, `delete_face`. If `e` is only in inner loops, replace the face under the same id without those inner loops (the hole closes; the face otherwise unchanged).
- Then `delete_edge(e)`, then `delete_vertex` for each end of `e` (in `ends` order) that has no edges left.
- Other edges are not merged or removed, even when two collinear edges are left meeting at a vertex.

## Tests (fixed; don't edit)
`tests/sticky_edges.rs` (snapping, crossings, T-junctions, overlaps, reuse, faces kept, split and erase, undo and redo of every edit) and `tests/sticky_edges_props.rs` (proptest: random segments on a grid, flat and 3D, with erases mixed in: the context validates clean after every step, the edges cover exactly the drawn segments, each step's log undoes and redoes exactly; random segments touching a grid of faces keep every face valid and unchanged in shape).
