# m7-solid-ui: solid status, problem marks and Repair in the app

Select a group or component and Entity Info says whether it is a solid and how big it is, or
what is wrong with it. The viewport draws the offending edges and faces in one problem color
(magenta), and a Repair button fixes what `m7-solid-repair` can fix in one undo step. The
checks are `modeler_kernel::solid::check_body` and `modeler_kernel::repair::repair_body`
(merged); this task is the display, the button and the plumbing.

Skeletons on main:
- `app/crates/modeler-dae/src/solid_marks.rs`: `SolidMarks`, `solid_marks`, `tint_problems`,
  `PROBLEM_COLOR`, `MARK_TOLERANCE_M` (re-exported from `modeler_dae`).
- `app/crates/modeler/src/solid_ui.rs`: `SolidStatus`, `solid_status`, `can_repair`,
  `format_volume`, `status_rows`, `repair_message`, `SolidCache`, `MarkTracker`, `marked_scene`.
- `Editor::repair_instance` (editor.rs), `InfoAction::Repair(InstanceId)` and
  `EntityInfoPanel::solid` (entity_info.rs), an empty `InfoAction::Repair` arm in `app.rs`.

Files to change: `modeler-dae/src/solid_marks.rs`, `modeler/src/solid_ui.rs`,
`modeler/src/editor.rs` (`repair_instance`), `modeler/src/entity_info.rs`, `modeler/src/app.rs`.

Acceptance tests: `modeler-dae/tests/solid_marks.rs`, `modeler/tests/solid_ui.rs`,
`modeler-render/tests/solid_marks_render.rs`. The existing `entity_info` tests must keep
passing unchanged (they are why one-shell parts show no Shells row).

Build on `m7-clearance-ui`: it replaces the three inline `tint_overhangs` uploads in `app.rs`
with `show_scene` (using `fit_check::display_scene`). If `show_scene` is not on main when you
start, stop and say so in your report; do not rework the upload path yourself.

## Design choices

- **Marks show whenever one group or component is selected and has something to show.** No
  menu item or toggle: a part with nothing wrong shows nothing, and seeing the marks is why you
  selected it.
- **Marks are dropped, not patched, when anything changes** (selection, edit path, undo
  state, a new document), as in `fit_check.rs`, and recomputed for the new selection.
- **Own geometry only.** Groups and components nested inside the selected one are counted
  ("Nested") but their problems are not marked; select and open them for that.
- **Cavities are not shown.** The check reports shells, not cavities; `Shells` covers it.
- **A component's copies share one definition**, so Repair changes every copy. That is how
  components behave; no extra warning.
- **The repair report is a note, not an error.** The editor's own message is drawn in the
  error color, so `Editor::repair_instance` leaves it empty on success and the app keeps the
  report text as a plain note.

## `solid_marks(model, body, instance, outer)` (modeler-dae)

- `instance` is looked up in `body`; its definition's body is checked with
  `check_geometry` on that body's own geometry (not `check_body`'s nested-instance rule).
  Missing instance, body or definition: empty marks.
- `triangles`: for each face in `reversed_faces` and `internal_faces` (once each, ascending
  id), its triangles as `kernel_scene` draws them: triangulate the face the way
  `draw_face` in `scene.rs` does (`polygon_normal`, `to_plane_2d`, `tess::triangulate`; a
  face with a zero normal has none), corners placed by `outer * instance.transform`.
- `segments`: the end points, placed the same way, of each edge in `open_edges`,
  `loose_edges` and `shared_edges` (ascending id within each, in that order).

## `tint_problems(scene, marks)` (modeler-dae)

Returns a new scene; `scene` is not changed. Same shape as `tint_overhangs`.

1. A scene triangle **matches** a mark triangle when each of its three corners (the `f32`
   positions as `f64`) is within `MARK_TOLERANCE_M` of a distinct corner of the mark, per
   axis, in any order. Both faces of a polygon (front and back triangles) match.
2. A scene line segment matches a mark segment when its two ends are within the tolerance
   of the mark's ends, either way round.
3. No match, or empty marks: the result equals `scene` (no material added).
4. Otherwise a material is appended (`id` "problem", `name` "Problem", `color`
   `PROBLEM_COLOR`, opaque, lit like the overhang one). Matching triangles move, in scene
   order, to one new batch at the end with that material (normals copied; texcoords
   dropped); a batch left without triangles is dropped. Matching segments leave their line
   batch and form one new line batch at the end with that material; an emptied line batch is
   dropped. `bounds` is kept.
5. Must not be quadratic: a scene of 100 000 triangles with 100 000 marks tints in well under
   a second in a release build (use a hash grid keyed by the quantized corner, looking at
   the neighbouring cells; the test allows 10 s in a debug build).

## `solid_ui` (modeler)

- `solid_status(model, body, selection)`: `selection` is exactly one entry, an
  `Entity::Instance` of `body`, and the instance's definition has a body. The report is
  `check_body(model, BodyId::Definition(definition))`.
- `can_repair`: `!report.is_clean()`. (A solid whose face is turned is solid but not clean.)
- `format_volume(m3, format)`: the unit is the format's (architectural: feet, 2 decimals);
  value `m3 / meters_per(unit)^3`, rounded to the format's decimals (at most
  `LengthFormat::MAX_DECIMALS`), trailing zeros and a bare point removed, then `mm³`, `cm³`,
  `m³`, `in³` or `ft³` with no space. Not finite counts as 0. This follows
  `units::format_area_with`.
- `status_rows(status, format)`, `(label, value)` in this order:
  1. `Solid` = `Yes` when `report.is_solid()`, else `No`.
  2. `Volume` = `format_volume(volume)`, when the report has a volume.
  3. Each of `Open edges`, `Loose edges`, `Shared edges`, `Reversed faces`,
     `Internal faces` (counts of the report's lists), `Nested` (`nested_instances`), only
     when the count is above 0, in that order.
  4. `Shells` = the count, only when it is not 1 (a lone shell is the normal case, and the
     existing panel tests look for the value `1` once).
- `repair_message(report)`:
  - Changed (`report.changed()`): `Repaired: ` then the non-zero parts joined by `, ` in
    this order: `removed N face(s)` (internal plus stray), `removed N edge(s)`,
    `filled N hole(s)`, `turned N face(s)`. "1 face", "2 faces"; "1 edge", "2 edges"; "1
    hole", "2 holes".
  - Not changed: `Nothing to repair` when `unfixed` is empty, else `Nothing could be
    repaired`.
  - When `unfixed` is not empty, append `; could not fix: ` and the kinds present, joined by
    `, `, as `label: n`, in this order: `open borders not forming one loop` (NotOneLoop,
    count of entries), `holes that aren't flat` (NotFlat), `ring-shaped holes` (Nested),
    `fills that aren't valid faces` (BadFill), `fills that would overlap a face` (Covered),
    `edges used by three or more faces` (SharedEdge), `groups or components inside`
    (NestedInstances: the sum of the numbers inside the entries).
- `SolidCache::get`: recompute only when `(model.cache_key(), body, selection)` differs
  from the last call (same pattern as `DescribeCache`); `computed()` counts recomputations.
- `MarkTracker::update`: as documented on the skeleton. The marks are
  `solid_marks(model, body, status.instance, path.transform(model))` with `body =
  path.body(model)`, `None` when empty, the path does not resolve or the selection is not one
  instance. Compare the new marks with the old to decide the return value.
- `marked_scene`: `Cow::Borrowed(scene)` for `None` (or empty marks); else
  `Cow::Owned(tint_problems(scene, marks))`.

## `Editor::repair_instance(model, instance)`

- `instance` must be an instance of the editor's open body (`path().body(model)`); else
  `Err("Select a group or component to repair")`, also stored as the message.
- `repair_body(model, BodyId::Definition(definition), &mut self.undo, "Repair Solid")`.
  Model refused: `Err("Repair failed")`, stored as the message. Success: clear the message
  and return the report. `repair_body` already pushes one undo step only when something
  changed.

## Entity Info panel

In `entity_info_panel`, when `panel.solid.get(model, body, selection)` gives a status (one
instance selected), after the existing Material / In model rows and before the size rows:
- one `value_row` per `status_rows` entry (label, value);
- a button labelled `Repair`, enabled only when `can_repair`; a click returns
  `InfoAction::Repair(status.instance)` (when no rename was returned the same frame).
Nothing is added for other selections (none, several, edges, faces).

## App wiring (`app.rs`; hand-checked, no window test)

- `App` holds a `MarkTracker`. A method `sync_solid_marks` calls `tracker.update(model,
  editor.path(), editor.selection(), undo_state)` (no kernel model: nothing) and returns its
  result. `show_scene` calls it first, then draws `marked_scene(&display, tracker.marks())`
  where `display` is what `fit_check::display_scene` returned, so the problem color wins
  over the overhang and fit tints. One upload path, as in `m7-clearance-ui`.
- Selection changes do not upload by themselves, so the per-frame update (where
  `sync_editor_path` runs) calls `sync_solid_marks` and, when it returns true, re-uploads with
  `show_current_scene`. A new or opened document calls `tracker.clear()` before its upload.
- `InfoAction::Repair(id)` in `apply_info_action`: `editor.repair_instance(model, id)`;
  on `Ok(report)` keep `repair_message(&report)` as a note with the selection and undo state
  after the repair, and call `refresh_document` when `report.changed()`. The note is drawn
  in the status bar as a plain label while the selection and undo state are still those; on
  `Err` the editor's message shows as usual (error color).

## Hand check (orchestrator, after merge, with `gui-session`)

Open a model with an open box, a box with a turned face and a clean box as groups: select each
and read Entity Info; see the magenta rim/face; press Repair; undo; confirm the note in the
status bar and that marks vanish and return.
