# m3-tool-tape: Tape tool state machine (kernel)

`app/crates/modeler-kernel/src/tools/tape.rs`: a `TapeTool` that turns clicks and typed offsets
into a distance reading or a guide. Like the other tools in `tools/`, no UI: the editor runs
inference, feeds `Input`s in and draws `preview`. Wiring is a later task. Tests:
`app/crates/modeler-kernel/tests/tool_tape.rs`.

Only `tape.rs` changes. Types and signatures are on main; fill in the bodies (private fields and
helpers are fine). The guide math is in `tape.rs` of the kernel (`parallel_guide`, `point_guide`,
`measure`; `docs/specs/m3-tape.md`), storing a guide is `Model::add_guide`
(`docs/specs/m3-guides-model.md`).

## Stages

`stage()` is `Start` (waiting for the start point) or `End`. A new tool is at `Start`, with no
last distance, no hover edge and the point-guide modifier off.

### Start

- `Move(p)`: `p` not finite: `Err(NotFinite)`. Otherwise nothing.
- `Click(p)`: `p` not finite: `Err(NotFinite)`, nothing changes. Then:
  - Point-guide modifier on: add `point_guide(p)` as one undo step labelled `"Guide point"`
    (`Err(Model(e))` if the model refuses). Stage stays `Start`; the last distance is kept.
  - Else the start is `p`, stage `End`, cursor forgotten. If the hover edge is set and longer
    than `LINEAR_TOL`, the measurement is **parallel** to that edge (`edge()` returns it);
    otherwise it is a plain measurement and `edge()` is `None`. Either way the hover edge is
    forgotten.
- `Typed(_)`: `Err(NotStarted)`.
- `Cancel`: nothing.

### End

- `Move(p)`: `p` not finite: `Err(NotFinite)`. Else remember the cursor.
- `Click(p)`: `p` not finite: `Err(NotFinite)`, nothing changes.
  - Plain: the last distance becomes `measure(start, p)`; stage `Start`; the model and undo
    stack are unchanged.
  - Parallel: the guide is `parallel_guide(a, b, p, None)` for the edge `(a, b)`. `None` (the
    point is on the edge's line): `Err(NoDirection)`, nothing changes. Otherwise add it as one
    undo step labelled `"Guide"`; the last distance becomes the guide's distance from the edge's
    line; stage `Start`.
- `Typed(s)`:
  - Plain: `Err(NotParallel)`.
  - Parallel: parse with `units::parse_length(s, target.unit)` (`Err(Parse(..))` on failure).
    A length of `LINEAR_TOL` or less: `Err(BadLength)`. No cursor yet, or the cursor on the
    edge's line: `Err(NoDirection)`. Otherwise the guide is
    `parallel_guide(a, b, cursor, Some(length))` (the cursor only picks the side), added like a
    click; the last distance is the typed length; stage `Start`.
- `Cancel`: stage `Start`; model and last distance unchanged.

A refused model edit (`Err(Model(e))`) leaves the tool, model and undo stack as before.

## Queries

- `start()`: the start point at `End`, else `None`.
- `edge()`: the edge at `End` of a parallel measurement, else `None`.
- `distance()`: at `End` with a cursor: plain, `measure(start, cursor)`; parallel, the cursor's
  distance from the edge's line (the same number `parallel_guide` would use for the offset).
  Else `None`.
- `last_distance()`: see above.
- `preview()`: at `End` of a parallel measurement with a cursor, `parallel_guide(a, b, cursor,
  None)`; else `None`.
