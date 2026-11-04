# m9-followme-tool: the Follow Me tool

Fill in `app/crates/drydock/src/follow_tool.rs` (skeleton on main: `FollowTool` with
`hint`, `click` and `apply` unimplemented; the constants and `Under` are final). Change only
that file. Everything else is already wired on main: `Tool::FollowMe`, the Tools > Follow Me
menu item, the status-bar hint (`Editor::follow_hint`), the `Editor` hooks that call
`FollowTool::click` / `apply` / `reset`, the preview (`Editor::preview` returns
`FollowTool::preview`, in body coordinates) and the clicks reaching the tool in the window.
Acceptance tests: `app/crates/drydock/tests/followme_tool.rs` (the editor driven with
pointer moves, clicks and keys, as `editor_pushpull.rs` does) and `tests/followme_ui.rs`
(the menu item, with kittest; these already pass).

Follow Me sweeps a flat profile face along a path of connected edges: a round profile along
a bent path makes a pipe, a moulding profile round a face's outline makes a frame. The
geometry is done: `sweep::path_from_edges`, `sweep::face_perimeter`,
`sweepface::sweep_sections` (no change to the model) and `sweepface::sweep_face`. This task
is the picking, the preview and the apply. No curve handling (`m9-sweep-curves`).

## Steps

The state is the profile, the path edges (a `Vec<EdgeId>`), the face a path came from, the
preview segments and a message. `click` is called for each left click with `Under` already
worked out by the editor: `face` is the face the ray meets, `entity` is what a Select click
would pick (an edge near the cursor wins over a face).

1. **Profile.** While there is no profile, a click takes `under.face` as the profile (nothing
   else: edges and empty space do nothing). It changes no geometry.
2. **Path, once there is a profile.** Look at `under.entity`:
   - `Edge(e)`: if `e` is one of the profile's outer-loop edges, refuse (message, nothing
     changes). Otherwise toggle it: not in the path, append it; already in, remove it. Any
     edge click forgets the "path face".
   - `Face(f)`: if `f` is the profile, refuse (message). If `f` is the current path face,
     this click **applies** (below) and returns its result. Otherwise the path becomes
     `face_perimeter(f).edges` (replacing whatever edges were there) and `f` is the path
     face. If it has no outline (`face_perimeter` fails), refuse with a message.
   - anything else (empty space, an instance): nothing.
3. **Preview.** After every click, recompute the segments. With a profile and a non-empty
   path that `path_from_edges` orders and `sweep_sections` accepts: for every section, every
   loop's consecutive point pairs (closing each loop); then, for each pair of consecutive
   sections and each loop, a segment from every point to the same-index point of the next
   section. In every other case the preview is empty (no error shown while previewing). The
   preview never touches the model or the undo stack.
4. **Apply** (`apply`, from Enter, or the second face click). With no profile or an empty
   path it does nothing and returns false. Otherwise order the edges with `path_from_edges`
   and call `sweep_face(model, body, profile, &path, undo, LABEL)`. On success: `reset()`
   (the tool is ready for the next sweep, hint back to the profile step) and return true. On
   any error from either call: return false, leave the profile, the path and the preview as
   they were, and set `message` to a short plain sentence for the user that says what was
   wrong (a gap or branch in the path, a profile that touches other faces, a turn too sharp,
   a profile parallel to the path, a path that twists the profile, ...). Do not print the
   error's `Debug` text. Write the message for `PathError` and `SweepError` variants one by
   one with a `match`.
5. **Hint** (`hint`): `HINT_PROFILE` with no profile; `HINT_PATH` with a profile and no path
   edges; `HINT_APPLY` once there are path edges.
6. `message` is cleared at the start of every `click` and `apply`. `reset` clears everything
   (the editor calls it on Esc and on a tool change).

The editor does the rest: Esc calls `reset`, and a tool change calls `reset`. One undo step
comes from `sweep_face` itself; the tool pushes nothing else.

## Tests

`followme_tool.rs`: hints through the steps and none for other tools; profile pick changes
nothing; empty space and an edge before the profile pick nothing; edges collect in click
order and a second click removes one; the profile and its edges are refused with a message;
a face click picks its outline and replaces earlier edges; the preview is empty until there
is a path, holds every section's loops and the joining lines, does not touch the model, and
is empty for a path with a gap; Enter applies an open path as one labelled undo step that
matches the kernel's own sweep, undo and redo work, and the tool is ready again; four edge
clicks sweep a closed ring; the path face's second click sweeps a ring; Enter with nothing
to apply does nothing; a gappy path is refused with a message, kept, and works once the stray
edge is clicked off; Esc drops everything; a tool change drops everything. `followme_ui.rs`:
the Tools menu item, its action, no toolbar button, the layout-file word.

## Not in scope

Curves staying curves (`m9-sweep-curves`); a toolbar button or shortcut; highlighting the
picked path edges in the viewport; preselecting the path with the Select tool; moving or
re-picking the profile after the path has begun (Esc and start again). The sign-off session
checks the tool in the real window.

## Where to look

- `follow_tool.rs`: `FollowTool`, `Under`, the constants (read the whole file, it is small)
- `sweep::SweepPath`, `sweep::PathError`, `sweep::path_from_edges`, `sweep::face_perimeter`
- `sweepface::sweep_sections`, `sweepface::sweep_face`, `sweepface::SweepError`
- `topology::Loop`, `topology::EdgeUse` (`face.outer.uses[i].edge`), `Context::face`
- `Model::body`, `Body::geometry`, `UndoStack`, `BodyId`, `select::Entity`
- Wiring, for reference only: `Editor::click` (the `Tool::FollowMe` arm), `Editor::key`
  (Enter and Cancel), `Editor::follow_hint`
- Test helpers in `followme_tool.rs`: `World::open`, `World::ring`, `reference`,
  `open_with_path`
