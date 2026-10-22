# m3-pushpull-merge: push/pull heals against its neighbours

Follow-up to `docs/specs/m3-pushpull.md` (read it first; every rule there still holds unless
changed here). Only `app/crates/modeler-kernel/src/pushpull.rs` changes; the signature and the
types stay. Tests: `tests/pushpull_merge.rs` (fixtures in `tests/pushpull_common/`). The old
tests in `tests/pushpull.rs` must keep passing.

Both rules below apply to the **extrude** path only, after the faces of the extrude exist and
before the "refusals after the edit" run, so those checks judge the healed result.

## Rule 1: sides merge with coplanar neighbours

For each new side face `S` in turn (list order), look at its bottom edge, the old edge `a`-`b`.
If that edge is now used by exactly two faces, `S` and one other face `G` that existed before
the push, and `G` is in the same plane as `S` with the same front direction (normals parallel,
dot > 0, `Plane`s the same within `LINEAR_TOL`), erase the edge with `Context`'s sticky
`erase_edge` (it merges two such faces and keeps one id). Merging only ever happens across that
bottom edge; the vertical edges between two sides stay even if the sides are coplanar.

Example: a rectangle drawn on a box top, touching the front edge, pulled up: the new front side
and the box's front wall become one face (the wall with a bump on top).

`PushPulled::sides` keeps one entry per use, in the same order; an entry whose side merged is
the id of the face now holding it (the survivor of the merge). `cap` and `base` as before.

## Rule 2: a cap landing on the opposite face punches a hole

After the extrude, if the cap lies in the plane of another face `F` of the body with the
opposite front direction (normals parallel, dot < 0, same plane within `LINEAR_TOL`), the cap
has no holes, and every cap vertex is strictly inside `F`'s outer loop and outside all of
`F`'s holes, then: delete the cap, and give `F` a new inner loop made of the cap's outer loop
(same uses, same order: it winds the right way for a hole in `F`). `PushPulled::cap` is then
`F` (the face now holding the hole). Example: the inset of a unit cube pushed in by exactly 1:
a square hole through the solid; the bottom becomes a ring.

If the cap lands on such a face but doesn't fit that description, do nothing special (the
usual refusals apply). Pushing past the opposite face is not handled here.

## Not here

A "new start face" modifier and merging collinear sides are later work.
Pushing a face in so that the pocket opens through a coplanar wall (its new side would face
opposite to the wall) is also later work: it stays refused with `Overlap`.
