# m4-dae-pinch-faces: split merged faces whose outline meets itself

Change one existing function in `app/crates/modeler-dae`: `to_kernel` (`src/kernel.rs`),
in the part that builds kernel faces from patches of triangles. No new public API.

Tests: `tests/kernel_pinch.rs` and `tests/kernel_pinch_corpus.rs` (corpus test, runs when
`MODELER_CORPUS` is set; the oracle sets it). `tests/kernel.rs` lost
`pinched_patch_falls_back_to_triangles` with this task (it pinned the old behaviour; the same
grid is `corner_pinch_gives_two_faces` now). Every other test must keep passing, including
`kernel_props.rs` and the corpus tests.

## Why

Import welds the file's triangles, groups flat, same-looking neighbours into patches, and
turns each patch into one face by tracing the patch outline. When that fails, the patch is
built as one face per triangle, so the model shows every triangle's edges in what should be
a plain flat surface. The common reason is an outline that meets itself:

- it passes the same vertex twice (a hole touching the outer outline or another hole at a
  corner, or two arms of the patch touching at a corner), which a kernel face may not do;
- it passes through a point twice without a shared vertex: a corner of the patch lies on
  an outline edge from elsewhere in the patch (exporters leave such "T" junctions).

Real models lose thousands of edges' worth of clean faces this way. After this task such a
patch becomes a few clean faces instead.

## What stays the same

Everything up to and including building patches (welding, dropping degenerate and duplicate
triangles, twins and back materials, texture maps per triangle, which edges are joinable, the
patch flood fill) is unchanged. A patch whose outline makes a valid face is still exactly one
face, built as today. Segments (lines) are still added as edges after the faces.

## When a patch can't be one face

Today: every triangle of the patch becomes its own face. New:

1. Split the patch into **regions** (below).
2. If there is more than one region, try to make each region one face, the same way a whole
   patch is made one face today: trace the region's outline (an edge of a region triangle is
   inside the region when it is joinable and the triangle across it is in the same region;
   every other edge is on the outline), outer loop first then holes, insert the face, and
   keep it only if the kernel reports no violation for it (delete it otherwise, as today).
3. A region that still can't be one face (and the single region of a patch that didn't
   split) falls back to one face per triangle, as today.

Each face gets the face materials of its first triangle (as today for a patch; every
triangle of a patch has the same materials, and placements that agree).

### Regions

Regions are grown greedily, in a fixed order so the result is deterministic:

- Take the patch's triangles in the order the patch lists them. The first one not yet in a
  region seeds a new region.
- Grow the region: look at every triangle across a joinable edge from a triangle already in
  the region. If it belongs to this patch, isn't in any region yet, and the region's outline
  stays **clean** with it added, add it. Keep scanning the region's triangles (including
  the ones just added); when a full pass adds nothing, the region is done. (A triangle
  refused earlier may fit later, once more of its neighbours are in, so keep passing until
  nothing changes.)
- Repeat with the next unassigned triangle until every triangle of the patch is in a region.

The region's outline is the set of directed outline edges: adding a triangle removes, for
each of its edges shared (joinable, other triangle in this region) with the region, the
reverse edge from the outline, and adds its other edges to it.

The outline is **clean** when, after adding triangle `t`:

- no corner of `t` starts more than one outline edge (so no vertex is passed twice);
- no outline corner of `t` (a corner that starts an outline edge) lies within `LINEAR_TOL`
  of an outline edge that doesn't end at that corner;
- no outline edge that `t` added passes within `LINEAR_TOL` of the start of another outline
  edge that isn't one of its own ends.

Distances are from the point to the closed segment. Checking against every outline edge is
fine: only patches that failed get here, and the whole corpus converts in a few seconds with
that.

A region that can't take a triangle leaves it for a later region, so a patch with one pinch
usually becomes two faces.

## Acceptance

`tests/kernel_pinch.rs`:

- a ring of cells pinched at one corner, a hole touching the outline, several pinches,
  holes touching each other: a small, stated number of faces, area kept, every face flat
  and facing up, the body valid;
- a patch whose outline touches itself along an edge (a corner lying on another part's edge,
  no shared vertex): split, every face valid;
- triangle order, determinism, few flat seams left between the pieces;
- materials: front and back materials and texture placement kept on every piece, and
  different materials never share a face;
- a property test over random flat grids with missing cells, either diagonal, any triangle
  order: valid, area kept, never more faces than cells.

`tests/kernel_pinch_corpus.rs`: on each corpus model, the number of "flat seams" (edges with
exactly two faces in one plane, facing the same way, with the same front and back
materials) is at most a bound per model. The bounds leave room above what a straightforward
implementation of the rules above gives; today's import is far above them.
