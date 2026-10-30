# m7-print-setup: Print Setup window, bed outline, fits-the-bed check, Drop to Bed

A Print Setup window (Tools > Print Setup...) holds the printer bed and the overhang angle.
The bed is drawn as an outline in the viewport at z = 0, a button checks whether the parts
fit on it, and Drop to Bed sets the selected parts on the bed. `m7-print-mesh` and
`m7-overhang` are merged; this task is the settings, the check, the window and the plumbing.

Skeletons on main (all bodies are `unimplemented!("m7-print-setup")`):
- `app/crates/modeler/src/print_setup.rs`: `Bed`, `BED_PRESETS`, `with_preset`,
  `parse_side_mm`, `parse_height_mm`, `format_mm`, `bed_outline`, `FitReport`, `fit_report`,
  `PrintSettings` (`parse`, `to_text`, `load`, `save`), `settings_file`, `PrintSetupView`,
  `PrintSetupEvent`, `PrintSetupTexts`, `print_setup_ui`. Every function's doc comment is
  part of this spec, including the exact message texts.
- `ViewState::set_overhang_degrees`, `ViewState::bed`, `ViewState::bed_visible`,
  `Action::PrintSetup`, `Effect::PrintSetup` (already returned by `ViewState::apply`), the
  Tools > "Print Setup..." menu item, and `Editor::drop_to_bed` and `Editor::select`
  (`select` is finished).
- An empty `Effect::PrintSetup => {}` arm in `app.rs`.

Files to change: `modeler/src/print_setup.rs`, `modeler/src/viewstate.rs`
(`set_overhang_degrees`), `modeler/src/editor.rs` (`drop_to_bed`), `modeler/src/app.rs`.

Acceptance tests: `modeler/tests/print_setup.rs`, `print_setup_ui.rs` and
`print_setup_render.rs` (helpers in `print_setup_common/`). Existing tests must keep passing.

## Design choices

- **Settings live in the user's config folder, not in the document.** A bed belongs to the
  printer, not the model, and the same folder already holds the recent-files list and the
  panel layout. No native format change. The file is `print-setup` (`settings_file`), read
  once at start and rewritten whenever a setting changes (a failed write is ignored).
- **The bed is centred on the origin** and lies at world z = 0 (not in the edit path's
  frame). Sizes are millimeters; the model is in meters, so the outline divides by 1000.
- **Fit means size, not place.** The check compares the extents of the box around the
  solids with the bed's sides. It does not look at where the parts stand and does not turn
  them to fit. The solids come from `print_mesh`, so only solid groups and components count
  and their curves are cut at the default tolerance.
- **Drop to Bed moves the selection as one.** All selected groups and components shift
  together by the same vertical amount, so their layout is kept and the lowest point of any
  of them lands on z = 0. A part below the bed comes up.
- **One window, no new dock panel.** It is an `egui::Window` like Model Info, so the dock
  layout file and `PANELS` do not change.

## `print_setup_ui` window and app wiring (`app.rs`)

- `Effect::PrintSetup` opens the window (a new `print_setup_open` flag, like
  `model_info_open`), titled "Print Setup", closable. The app keeps a `PrintSettings`
  (loaded at start from `settings_file(std::env::var_os)`), a `PrintSetupTexts`, and the last
  `FitReport`. At start `ViewState::bed`, `bed_visible` and `overhang_degrees` are set from
  the loaded settings.
- Each frame the window calls `print_setup_ui` with a `PrintSetupView` built from the view
  state. `can_drop` is true when the editor's selection holds an `Entity::Instance`.
  The event:
  - `SetBed(bed)`: store it in `view.bed`, rewrite `texts` with `PrintSetupTexts::for_view`,
    save the settings, forget the last report;
  - `ShowBed(on)`: `view.bed_visible = on`, save;
  - `SetOverhang(d)`: `view.set_overhang_degrees(d)` and handle the returned effect as the
    app handles `Effect::ReuploadScene`, save;
  - `CheckFit`: `fit_report(model, instances, &view.bed)` where `instances` is the selected
    instances' ids when the edit path is the root and the selection holds at least one
    instance, else `None` (whole model); keep the report and show its `message` in the status
    bar as a note (not the red error text);
  - `DropToBed`: `editor.drop_to_bed(model)`, then refresh the document as `Effect::Flip`
    does; forget the last report.
- A report is forgotten too when the document, the selection or the undo state changes
  (as `fit_check` and the repair note do), so the window never shows a stale answer.
- The bed outline: in `paint_viewport`, when `view.bed_visible`, append
  `print_setup::bed_outline(&view.bed, pixels_per_point)` to the extra lines next to the axes.
  They are drawn whether or not a document has geometry.
- Hand-checked after merge by the orchestrator in the real window; no test drives
  `app.rs`.

## `Editor::drop_to_bed`

- Acts on the `Entity::Instance` entries of the selection (others are ignored). With none:
  message `"Select groups or components to drop to the bed"`, nothing changes, `false`.
- The lowest point is the smallest world z over `modeler_dae::instance_triangles(model, body,
  id, world)` of those instances (`world` is the open body's transform, identity at the root).
  Curves count by their vertices.
- Already on the bed (`|lowest| < 1e-9` m): no change, no undo step, message cleared, `false`.
- Otherwise translate the instances by `(0, 0, -lowest)` in world space with
  `transform::transform` (matrix conjugated by the open body's transform so it is correct
  inside an open group), as one undo step labelled "Drop to Bed"; clear the message; `true`.

## `ViewState::set_overhang_degrees`

As its doc comment says. The View > Overhangs toggle and the stored angle stay as they are;
only the window changes the angle.

## Tests and verification

- The tests cover every pure function, the settings text, the view state, the menu item,
  Drop to Bed through `Editor`, the window with `egui_kittest`, and the outline in a headless
  render. A throwaway implementation passes all of them.
- Do not change the tests, `Cargo.toml` or the skeleton's signatures. If a doc comment and a
  test seem to disagree, stop and say so.

## Where to look

- `harness/bin/show chrome::clearance_field` and `tests/fit_check_ui.rs`: the commit-on-blur
  text field this window copies (`print_setup_ui`).
- `harness/bin/show overlay::axis_lines`, `App::paint_viewport` in `app.rs`: extra lines.
- `harness/bin/show Editor::flip` and `Effect::Flip` in `app.rs`: a one-step transform of the
  selection and the refresh after it.
- `recent::config_dir`, `RecentFiles::save`, `RecentFiles::load`: the settings file pattern.
- `modeler_mesh::print_mesh::print_mesh`, `PrintScope`, `PrintObject`: the fit check's triangles
  (millimeters, world space). `modeler_dae::instance_triangles`: Drop to Bed's triangles.
- `Effect::ReuploadScene` handling in `app.rs`, `Model Info` window code (`model_info_open`,
  `chrome::model_info_window_with`): how a window is opened and drawn.
- `tests/print_setup_common/mod.rs`: `add_box`, `box_mm`, `lowest_z`.
