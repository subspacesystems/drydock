# m9-sweep-path: edges into an ordered sweep path (kernel)

Implement `app/crates/drydock-kernel/src/sweep.rs` (skeleton on main: `SweepPath`,
`PathError`, `SweepPath::reversed`, `SweepPath::length`, `path_from_edges`,
`face_perimeter`). Change only that file. Acceptance tests:
`app/crates/drydock-kernel/tests/sweep_path.rs` and `tests/sweep_path_props.rs`.

Follow Me sweeps a profile face along a path. The user gives the path as a set of selected
edges, in no particular order, or as a face whose outline is the path (to turn a profile all
the way round a face, like a lathe). This task turns either into one ordered chain of
vertices. The sweep itself (`m9-sweep-face`) and the tool (`m9-followme-tool`) come later and
build on it. Nothing here changes the context: both functions take `&Context`.

## `SweepPath`

- `vertices`: the chain's vertices in order; `points[i]` is the position of `vertices[i]`.
- `edges[i]` joins `vertices[i]` and `vertices[i + 1]`; when `closed`, the last edge joins the
  last vertex back to `vertices[0]`.
- Open: one more vertex than edges. Closed: as many vertices as edges; `vertices[0]` is not
  repeated at the end. No vertex appears twice.
- `length()`: the sum of the edges' lengths (distance between the points each edge joins).
- `reversed()`: the same path run the other way. Open: vertices, points and edges all in
  reverse order. Closed: `vertices[0]` (and `points[0]`) stays first, the other vertices
  follow in reverse order, and the edges are in reverse order. `closed` is unchanged.
  Reversing twice gives the original.

## Path from edges: `path_from_edges(ctx, edges)`

The order of `edges` and any repeats don't matter: the same set gives exactly the same
path (`==`), however the edges were created or which way each one is stored.

1. **Unknown edges.** If an edge in `edges` is not in `ctx`, return
   `UnknownEdge(that edge)` for the first such one in input order. This comes before every
   other check.
2. **Empty.** No edges given: `Empty`.
3. **Curves.** Start from the set of given edges. For every curve in `ctx`
   (`Context::curves`) whose step edges (`curve::curve_edges`) include a given edge, add all
   of its step edges. (Selecting one segment of an arc or circle means the whole arc.) Only
   the given edges trigger this; edges added from one curve don't pull in another curve.
   A curve whose steps are broken has no step edges and adds nothing.
4. **Branches.** Count, for each vertex, the edges of the set that end there. If any vertex
   has three or more, return `Branch(v)` for the lowest such vertex id. Edges of the context
   that are not in the set don't count.
5. **Gaps.** Edges are connected when they share a vertex (the same `VertexId`; two
   vertices at the same position are not connected). If the set falls into two or more
   connected pieces, return `Gap { pieces }` with their number. (Branches are checked first:
   a set with both reports the branch.)
6. **Order.** The set is now one chain where every vertex has one or two edges.
   - **Open** (some vertex has only one edge; there are exactly two such ends): start at the
     end with the lower vertex id and walk the chain to the other end.
   - **Closed** (every vertex has two edges): start at the lowest vertex id and walk first
     along the lower-id one of its two edges, then round until the next step would return
     to the start.
   - `points` come from the vertices' positions in `ctx`.

## Path around a face: `face_perimeter(ctx, face)`

- `UnknownFace(face)` if the face is not in `ctx`.
- Otherwise the face's outer loop as a closed path, in the loop's own direction (each use's
  start vertex, `Context::use_start`, in order: counter-clockwise seen from the front). The
  path starts at the loop's lowest vertex id; the loop is rotated, not reversed, to begin
  there. `edges` are the loop's edges in the same rotated order.
- Inner loops (holes) are ignored.
- Curve expansion doesn't apply; the loop already has every edge.

So for a face's outer-loop edges, `path_from_edges` gives either `face_perimeter` or its
`reversed()`.

## Tests

- `sweep_path.rs`: open chains (single edge, in and out of order, repeats, edges stored
  backward, start at the lower-id end wherever it lies, edges not given are left out);
  closed chains (start vertex and first edge, triangle, a face's edges vs its perimeter);
  errors (empty, first unknown edge in input order and before other errors, branch at the
  lowest vertex, branches count only given edges, gaps with their count, coincident but
  distinct vertices are a gap, branch before gap); curves (one step brings a whole arc or
  circle, arcs and lines in one chain, a line touching an arc doesn't bring it, curve steps
  making a branch, two arcs each given by one step); perimeter (start vertex, loop
  direction kept for a loop that uses its edges backward, holes ignored, a circle face,
  unknown face); reversing open and closed paths.
- `sweep_path_props.rs`: random open and closed chains with random vertex and edge creation
  order, stored directions, input order and repeats give a well-formed chain over exactly
  the given edges, with the canonical start, equal to the path from the edges in id order;
  `reversed` keeps a well-formed chain and its length and undoes itself; dropping an inner
  edge (open) or two non-touching edges (closed) gives `Gap { pieces: 2 }`; a spur edge on
  an inner vertex gives `Branch`, on an open end just lengthens the chain.

## Not in scope

Building any geometry from the path (`m9-sweep-face`); choosing which end the sweep starts
from or where a closed path starts relative to the profile (the sweep can use `reversed` and
rotate the path itself); merging collinear edges; picking the path in the UI
(`m9-followme-tool`); smooth side faces along curves (`m9-sweep-curves`).

## Where to look

- `Context::edge`, `Context::vertex`, `Context::face`, `Context::curves`,
  `Context::use_start`, `Context::loop_vertices`, `Context::edges_of_vertex`
- `curve::curve_edges` (a curve's step edges, in chain order)
- `Edge` (`ends`), `EdgeUse`, `Loop`, `Face`, `VertexId`, `EdgeId` (ids are `Ord`)
- Test helpers in the tests themselves: `chain`, `arc`, `square_face`, `assert_path`
  (`sweep_path.rs`), `build`, `check_chain` (`sweep_path_props.rs`)
