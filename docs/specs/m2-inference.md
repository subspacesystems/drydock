# m2-inference: snaps, axis inference, direction lock, picking

Module `modeler-kernel/src/infer.rs` (skeleton on main). Pure functions over one `Context`, in its own coordinates. Instances are not entered. No new dependencies. Never panic.

## Terms
- `d̂` = `probe.dir` normalized. If `dir` is zero or not finite, or `origin` or `cursor` is not finite, `infer` and `pick` return `None`.
- Depth of a point `p`: `(p - origin) · d̂`.
- Screen distance of `p`: `|project(p) - cursor|`. A point whose `project` is `None` has no screen distance and is never a candidate.
- A candidate is *in range* when its screen distance is `<= radius` (a negative or NaN radius puts nothing in range).
- `TOL` = `geom::LINEAR_TOL`.

## `ray_face(ctx, origin, dir)`
- `None` for a zero or non-finite `dir`. Use the unit ray `origin + t·d̂`.
- For each face: its plane is `ctx.face_plane`; skip faces with no plane and faces seen edge-on (`|d̂ · normal| < 1e-12`). `t` = distance along the ray to the plane; it must be `> 1e-9`.
- The hit point must be inside the face: map the hit point, the outer loop and each hole to 2D with `geom::to_plane_2d` (the face plane's normal); `point_in_polygon_2d` with tolerance `1e-9` must be `Inside` or `Boundary` for the outer loop and not `Inside` for any hole (a hole's boundary counts as on the face).
- Return the face with the smallest `t` and that `t`; ties go to the lower `FaceId`.

## Hidden points
A candidate point `p` belonging to entity `E` is hidden when a face that doesn't touch `E` blocks the view of it:
- Sight line start `s` = `origin` for a perspective view, `p - d̂·depth(p)` for a parallel one. `L` = `|p - s|`. If `L <= TOL`, `p` is not hidden.
- Faces that touch `E`: for a vertex, every face of every edge at the vertex; for an edge (midpoint or on-edge point), the faces of that edge. Centers and axis points touch no face.
- `p` is hidden when `ray_face`-style hits (same inside test) from `s` toward `p`, over the faces that don't touch `E`, include one with `1e-9 < t < L - TOL`.

## Candidates
Candidates 1-5 also need `depth(p) > 0` and must not be hidden.
1. **Endpoint**: each vertex of the context, `p` = its position. Id `VertexId`.
2. **Midpoint**: each edge, `p` = the exact midpoint `(a + b) / 2`. Id `EdgeId`.
3. **Center**: each `probe.centers[i]`, `p` = that point. Id `i`.
4. **OnEdge**: each edge `a`-`b`: `p` = the point of the segment closest to the ray's line (minimize over the segment parameter `s` in `[0, 1]`: solve the two-line closest points, clamp `s`). When the edge is parallel to the ray (`|d̂ × (b - a)| <= 1e-12·|b - a|`), `p` is the end with the smaller depth (`a` on a tie). Id `EdgeId`.
5. **OnAxis**: only with an `anchor` and no active lock. For each axis `i`: the line `anchor + s·axes[i]` (any `s`); `p` = the point of that line closest to the ray's line. Skip axes parallel to the ray (`|d̂ × axes[i]| < 1e-9`). Id `i`.

Candidates 1-5 must be in range. Then:

6. **OnFace**: `ray_face(ctx, origin, dir)`; `p` = `origin + t·d̂`.
7. **Ground**: the plane through `anchor` (or through the world origin without an anchor) with normal `axes[2]`. The ray must not be parallel to it (`|d̂ · axes[2]| >= 1e-12`) and must meet it at `t >= 0`; `p` = that point.

## `infer(ctx, probe)`
Without an active lock: take the first category in the order above that has a candidate. Within a category, choose by smallest screen distance, then smallest depth, then lowest id. Categories 6 and 7 have at most one candidate each. `snap` names the category and id; `point` = the candidate's `p`. `None` when no category has a candidate.

**Lock.** The lock is active when `anchor` is `Some` and `lock` is a finite non-zero vector; otherwise `lock` is ignored. Let `â` be the lock normalized and the lock line `anchor + s·â`.
- Look for an Endpoint, Midpoint or Center candidate as above (categories 1-3 only, same order and tie rules). If one is found, `point` = its `p` projected onto the lock line (`anchor + ((p - anchor) · â)·â`) and `snap` = that candidate's snap.
- Otherwise `snap` = `Along` and `point` = the point of the lock line closest to the ray's line; if the lock line is parallel to the ray (`|d̂ × â| < 1e-9`), `point` = `anchor`.
- An active lock always gives `Some`.

**axis.** `Some(i)` when there is an anchor, `|point - anchor| > TOL` and `|u · axes[i]| >= 1 - 1e-9` for `u` = `(point - anchor)` normalized; the lowest such `i`. Otherwise `None`. (An `OnAxis(i)` result has `axis` = `Some(i)`.)

## `pick(ctx, probe)`
The OnEdge candidate (category 4: in range, depth > 0, not hidden) with the smallest screen distance, then smallest depth, then lowest id, as `Entity::Edge`. Otherwise the `ray_face` face as `Entity::Face`. Otherwise `None`. `anchor`, `centers` and `lock` don't matter.

## Notes
- Only `infer.rs` changes (plus private helpers inside it). Reuse `geom` (`to_plane_2d`, `point_in_polygon_2d`, `LINEAR_TOL`) and `Context` queries (`face_plane`, `loop_points`, `edges_of_vertex`, `faces_of_edge`).
- Cost: a plain scan over vertices, edges and faces per query is fine for now.
