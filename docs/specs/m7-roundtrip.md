# m7-roundtrip: print round trip and the own mesh validator

Implement `validate` and `MeshReport::is_valid` in `app/crates/modeler-mesh/src/validate.rs`
(skeleton on main). Change only that file. Acceptance tests: `tests/mesh_validate.rs` (the
validator alone) and `tests/roundtrip.rs` (files written and read back). The round-trip tests
also need the STL and 3MF writers, readers, `print_mesh` and `repair_body`, which are merged
before this task runs; they need nothing else from you.

The validator is the project's own check that a triangle list is printable, independent of the
kernel's solid check. It is wired into the oracle simply by being used in these tests: the
oracle's `test` gate runs them, so there is no new gate and no script.

## validate(triangles, weld_mm)

Triangles are in millimeters, corners counter-clockwise seen from outside.

1. **Weld.** A point's key is `((x / weld_mm).round(), (y / weld_mm).round(), (z / weld_mm).round())`
   as integers. Equal keys are the same point. `vertices` counts distinct keys over all
   triangles, degenerate ones included.
2. **Degenerate triangles.** A triangle is degenerate if two of its keys are equal, or its area
   (half the cross product of two edges, from the original coordinates) is under 1e-9 times its
   longest edge squared. Count them in `degenerate_triangles`; they are left out of the edge
   counts and the volume.
3. **Edges.** For each other triangle, its three edges run key to key. Count, per undirected
   edge, the uses that run low-to-high and high-to-low (compare keys in any fixed order).
   - one use in total: `open_edges`;
   - three or more in total: `non_manifold_edges`;
   - exactly two, both running the same way: `inconsistent_edges`;
   - exactly two running opposite ways: fine.
4. **Volume.** The sum over the other triangles of `a . (b x c) / 6` from the original
   coordinates (positive when facing outward).
5. **`is_valid`.** `triangles > 0`, and `degenerate_triangles`, `open_edges`,
   `non_manifold_edges` and `inconsistent_edges` all 0, and `volume > 0.0`.

`triangles` is the number of triangles given. An empty list gives an all-zero report that is
not valid.

## What the round-trip tests check

- Closed solids (box, plate with a hole, faceted and circle-edged cylinders, a rotated part, three
  parts of which two share a component) go through `print_mesh`, a writer, the reader,
  `to_kernel` and `print_mesh` again: valid mesh both times, same volume (1e-6 relative), same
  bounding box, same object count and, for 3MF, the same names.
- Broken models (a reversed face, an internal wall, a missing face with a flat hole, a loose
  edge) are found by `check_body`, skipped by `print_mesh` while broken, fixed by `repair_body`
  in one undo step, and then export like the good model.

If a round-trip test fails because a writer or reader is wrong, do not edit them here: say so
in your final note and leave the test failing (the task goes back to the orchestrator).

## Where to look

- `show validate`, `show MeshReport` (skeleton, `modeler-mesh/src/validate.rs`).
- Tests' helpers: `tests/print_common/mod.rs` (`box_group`, `box_group_turned`,
  `open_box_group`, `plate_with_hole`, `cylinder_group`, `volume`, `bounds`).
- `show print_mesh`, `show Triangle` for the input type; `show check_body`, `show repair_body`
  for what the broken-model tests call (you don't change them).
