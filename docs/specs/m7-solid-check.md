# m7-solid-check: is a group a printable solid? (kernel)

Implement `app/crates/modeler-kernel/src/solid.rs` (skeleton on main: `SolidReport`,
`check_geometry`, `check_body`, `winding_number`). Change only that file. Acceptance tests:
`app/crates/modeler-kernel/tests/solid_check.rs` and `tests/solid_check_props.rs` (they use
the shared helpers in `tests/transform_common/`).

A 3D printer needs each part to be a closed surface that separates inside from outside. This
task reports, for one context (the root or a group/component definition), everything that
keeps it from being one. Later tasks repair it (`m7-solid-repair`), show it in Entity Info
(`m7-solid-ui`) and export it.

## Terms

- **Uses of an edge**: the faces whose outer or inner loops contain the edge. (A valid face
  uses an edge at most once.) `Context::faces_of_edge` gives them.
- **Direction of a use**: the `EdgeUse::reversed` flag of the edge in that face's loop.
- Two faces sharing an edge that exactly two faces use are **consistent** when they run the
  edge in opposite directions (one forward, one reversed), and **inconsistent** otherwise.
- **Area vector** of a face: the sum of `geom::area_vector` of its outer loop and of every
  inner loop (inner loops run the other way, so holes subtract). It points out of the front;
  its length is the face area.
- **Signed volume** of a set of faces, each with a front: the sum over faces and over each of
  their loops `l = [p0, p1, ..., pn-1]` of `p0 · (pi × pi+1) / 6` for `i` in `1..n-1`; a
  face counted with its back as front adds the negative. For a closed surface whose fronts
  all point out this is the enclosed volume.
- **Winding number** of a set of faces around a point `p`: the sum of the signed solid
  angles of the same fan triangles `(p0, pi, pi+1)` of every loop, divided by `4π`. For a
  triangle with corners `a, b, c` relative to `p` (`a = p0 - p`, ...), the signed solid
  angle is `2 · atan2(a · (b × c), |a||b||c| + (a·b)|c| + (a·c)|b| + (b·c)|a|)`. A face
  counted with its back as front adds the negative.

## `check_geometry(ctx)`

1. **Edge classes.** For every edge, count its uses: 0 → `loose_edges`, 1 → `open_edges`,
   3 or more → `shared_edges`. Two uses is the normal case and goes in no list.
2. **Internal faces.** A face is internal when every edge of every one of its loops is a
   shared edge. (A wall dividing a box, or the face two stacked boxes share.) Topology only:
   don't intersect faces with each other or classify them by position.
3. **Shells.** Faces are joined when they share an edge with exactly two uses. `shells` is
   the number of connected groups of faces (internal faces form their own groups like any
   other face). A context with no faces has 0 shells.
4. **Orientation inside a shell.** Walk each shell breadth first from its lowest face id,
   visiting neighbours in ascending face id order; the start keeps its stored front. A newly
   reached face `g`, reached from `f` across edge `e`, is *turned* exactly when `f`'s and
   `g`'s stored uses of `e` run the same direction XOR `f` is turned (so `g`'s walked front
   is consistent with `f`'s walked front). A face reached again later is not changed.
   This gives each face a front such that the shell runs every joined edge both ways.
5. **Which way out.** A shell is **closed** when every edge of its faces has exactly two uses
   and both are in the shell.
   - For an *open* shell, the side of the walk with the larger total face area is taken as
     right: if the turned faces' area exceeds the unturned faces' area by more than `1e-9`
     times the shell's total area, the unturned faces are reversed; otherwise (including a
     tie, which keeps the side of the shell's lowest face) the turned faces are.
   - For *closed* shells, first find each one's **depth**: the number of other closed shells
     it lies inside. Shell `S` lies inside closed shell `T` when `|winding number of T's
     faces (with their walked fronts) around q| > 0.5`, where `q` is a test point of `S`:
     triangulate `S`'s lowest-id face (`tess::triangulate` on the loops projected with
     `geom::to_plane_2d` along its plane normal) and take the centroid of the triangle with
     the largest area, mapped back to 3-D as `x·u + y·v + (n·p)·n` (`(u, v) =
     geom::plane_basis(n)`, `n` the face's unit area vector, `p` any vertex of the face). A shell at even depth (0, 2, ...) is material
     around its inside, so its fronts must point out: its signed volume (walked fronts) must
     be positive. A shell at odd depth is a cavity: its signed volume must be negative. If
     the sign is wrong, every face of the shell flips its final front. Then the reversed
     faces are again those whose final front differs from the stored one.
   `reversed_faces` collects them from every shell.
6. **Volume.** When `is_solid()`, `volume` is the signed volume of all faces with their final
   fronts (outer shells add, cavities subtract). Otherwise `None`.

`nested_instances` is 0.

## `SolidReport::is_solid` / `is_clean`

`is_solid`: `shells >= 1`, `open_edges`, `loose_edges`, `shared_edges` and `internal_faces`
empty, `nested_instances == 0`. Reversed faces don't stop a solid (the volume counts them
the right way round); `is_clean` is `is_solid` with no reversed faces either. Both read the
fields only, so a report built by hand answers the same.

## `check_body(model, body)`

`None` when the body doesn't exist. Otherwise `check_geometry` of its geometry with
`nested_instances` = the number of instances in that body; a body with nested instances is
not solid, so `volume` is then `None` too.

## `winding_number(ctx, faces, point)`

The winding number above for the listed faces with their stored fronts. Ids not in `ctx` are
skipped. Step 5 needs the same sum with some faces turned; share the code.

## Notes

- Zero-area faces can't occur: `Context::validate` already refuses loops with area under
  `LINEAR_TOL²`.
- Speed: `Context::faces_of_edge` is a map lookup; look each edge up once. The winding
  test is O(closed shells² · faces); fine for printable parts.
- Pure function: nothing in the model changes.

## Not in scope

Repair (`m7-solid-repair`), world-space volume of an instance (the UI multiplies by the
transform's determinant), faces that pierce each other without an edge between them (not
detected; the volume of such a part is whatever the signed sum gives).
