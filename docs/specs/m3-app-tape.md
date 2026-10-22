# m3-app-tape: Tape Measure in the editor

The editor (`app/crates/modeler/src/editor.rs`) gets a Tape Measure tool that drives the kernel's
`modeler_kernel::tools::tape::TapeTool` (read `docs/specs/m3-tool-tape.md`: start on an edge for
a parallel guide, otherwise a plain measurement, typed offsets, the point-guide modifier). The
editor turns cursor events into points and into the hovered edge, and keeps messages and the
measurement box. No window code here: toolbar, keys and drawing the guides come in a later task.
Tests: `app/crates/modeler/tests/editor_tape.rs`. The other editor tests must keep passing.

Only `editor.rs` changes. `Tool::Tape`, the `tape: TapeTool` and `tape_point_guides` fields,
`Editor::tape_tool` and `Editor::set_point_guides` (both done) and `unimplemented!` arms are on
main; fill the arms in. Root body only. Read `docs/specs/m3-app-move.md`: Tape follows the same
shape (typed text, idle on Esc/undo/redo/tool switch) without a selection.

## Points and the hovered edge

Run inference as the Move tool does (`Probe::anchor` = `tape.start()`, `lock` = `None`). The
point is `inf.point`; the hint is `inf`. No inference: hint `None`, nothing is sent to the tool.

`pointer_move` and `click` both refresh the hovered edge first, while the tool is at
`TapeStage::Start`: pick under the cursor as the Select tool would. If the entity is an
`Entity::Edge` of the root body, `tape.set_hover_edge(Some((a, b)))` with the positions of its
ends; anything else (a face, an instance, nothing): `set_hover_edge(None)`. After the start
click the tool keeps its own edge.

`pointer_move`: the tool gets `Input::Move(point)`.

## Clicks

`click(.., count)`: `count` is ignored. Before the click, `tape_point_guides` is passed to
`tape.set_point_guides`. The tool gets `Input::Click(point)` with a `Target` on the root body
with the editor's undo stack and unit. `Ok` clears the message; `Err` sets it (wording below).

Returns true exactly when an undo step was pushed (a guide or guide point was added). Starting
or finishing a plain measurement returns false.

## Typed offset

`text(s)` takes text for Tape with the same rules as the drawing tools. `Enter` with typed text
sends `Input::Typed(text)`, then clears the text; `Ok` clears the message, `Err` sets it.
Returns true when an undo step was pushed.

## Messages

| Error | Message |
| --- | --- |
| `Parse(_)` | "Not a length" |
| `NotStarted` | "Click a start point first" |
| `NotParallel` | "Start on an edge to place a guide" |
| `NotFinite` | "Can't measure that" |
| `NoDirection` | "Move the cursor to show the side" |
| `BadLength` | "Length must be more than zero" |
| `Model(_)` | "Can't add that guide" |

## Other keys and tool switches

- Esc (`Cancel`): the tape tool gets `Input::Cancel`; typed text and message cleared. The model
  is unchanged.
- Undo / Redo: the tape tool gets `Input::Cancel` first, then as for the other tools (so undo
  removes the last guide, redo brings it back).
- `set_tool` (to any tool): the tape tool is replaced by a new one (the last distance is
  forgotten). `set_point_guides` is kept.
- Arrow keys and `draw_plane()`: nothing for Tape (`draw_plane()` is `None`).

## Output

- `measurement()`: label `"Length"`, lengths formatted like Move's (`units::format_length` with
  the editor's unit, 2 decimals). Value: at `End`, `tape.distance()` if `Some`, else `""`; at
  `Start`, `tape.last_distance()` if `Some`, else `""`.
- `preview()`: nothing at `Start`. At `End`: for a plain measurement (`tape.edge()` is `None`),
  the segment from `tape.start()` to the hint point (nothing if there is no hint). For a
  parallel measurement, if `tape.preview()` is a `Guide::Line { point, dir }`, the segment
  between the ends of the edge projected onto that line: for each end `p` of the edge,
  `point + dir * dir.dot(p - point)`.
- `tape_tool()`: `Some(&tape)` while the tool is Tape (on main).
- `hint()`: as in Points.
