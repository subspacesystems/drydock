# m2-tool-line: tool plumbing and the line tool

Module `modeler-kernel/src/tools/` (skeleton on main: `mod.rs`, `line.rs`). No new dependencies. Never panic. The tools don't run inference: the app turns the cursor into a point (with `infer`, using the tool's `anchor`) and passes it in as `Input`.

## Terms
- `TOL` = `geom::LINEAR_TOL`. A point is *finite* when all three coordinates are.
- An *edit* below means: a fresh `ModelEdit`; changes go into it through `Model::edit_geometry` (with `Context::add_edge`) and `Model::set_face_materials`; when it's done and non-empty it is pushed with `target.undo.push(label, edit)`.
- "Nothing changes" means the model, the undo stack and the tool's own state are as before the call.

## `draw_segments(target, segments, label)`
1. Start one edit. For each segment `(a, b)` in order, `add_edge(a, b)` in `target.body`'s geometry. If one fails, undo what this call did so far (`apply_edit` of the edit's inverse), return `ToolError::Geometry(e)`; nothing changes. A missing body is `ToolError::Model(ModelError::MissingBody(body))`, nothing changes.
2. Material copy: walk all the chains' `faces` lists in order (first segment's chain first). For each `(new, Some(old))`: if the body's `face_materials(old)` is not the default, `set_face_materials(new, that)` in the same edit. (The old face is gone by then but its entry stays in the body. A face split from a face created earlier in the same call gets the materials just copied to that one.) `(new, None)` faces get nothing.
3. Push the edit with `label` (an empty edit is not pushed: `UndoStack::push` drops it). Return the chains, one per segment, in order.
- An empty `segments` returns `Ok(vec![])` and changes nothing.

## `LineTool`
State: *idle*, or *drawing* from a start point `s`, with an optional cursor point `c` (the last `Move`).

- `new()`: idle.
- `anchor()`: `Some(s)` while drawing, else `None`.
- `preview()`: `Some((s, c))` while drawing with a cursor `c` and `|c - s| > TOL`; else `None`.
- `length()`: `Some(|c - s|)` exactly when `preview()` is `Some`, else `None`.

`handle(input, target)`:
- **Move(p)**: a non-finite `p` is ignored (`Ok`, nothing changes). Idle: `Ok`, nothing changes. Drawing: `c = p`; `Ok`.
- **Click(p)**:
  - Non-finite `p`: `Err(Geometry(StickyError::NotFinite))`, nothing changes.
  - Idle: start drawing at `s = p`, no cursor. `Ok`. The model is not touched.
  - Drawing, `|p - s| <= TOL`: `Ok`, nothing changes.
  - Drawing otherwise: *place* `p` (below).
- **Typed(text)**:
  - Idle: `Err(NotStarted)`, nothing changes (the text is not parsed).
  - Drawing: `units::parse_length(text, target.unit)`; on error `Err(Parse(e))`, nothing changes. Then the direction `u` = `(c - s)` normalized; with no cursor, or `|c - s| <= TOL`, `Err(NoDirection)`, nothing changes. Otherwise place `s + u·len` (a negative length goes the other way).
- **Cancel**: idle, no cursor. `Ok`. The model is not touched.

**Placing** a point `q`: `draw_segments(target, &[(s, q)], "Line")`. On error, return it; nothing changes (still drawing from `s`, same cursor). On success, with the one chain `ch`:
- If `ch.faces` is non-empty (the edge closed a face), go idle.
- Otherwise keep drawing from the position of the vertex `ch.vertices.last()` (where the edge really ended after snapping), with no cursor.
- Return `Ok`.

Each placed edge is its own undo step labelled `"Line"`.

## Notes
- Change only `tools/mod.rs` and `tools/line.rs` (add private fields and helpers as needed; keep the public items' signatures).
- Reuse `Context::add_edge`, `Model::edit_geometry`, `Model::set_face_materials`, `Model::apply_edit`, `ModelEdit::inverse`, `Body::face_materials`, `units::parse_length`.
- Rectangle, circle and arc tools come in a later task and will use `draw_segments`, `Input`, `Target` and `ToolError`.
