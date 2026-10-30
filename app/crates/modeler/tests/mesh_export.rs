// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-export-ui: File > Export 3MF... and Export STL..., the export function and the dialog.
//! See docs/specs/m7-export-ui.md.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::chrome::{MenuState, menu_bar};
use modeler::input::Action;
use modeler::mesh_export::*;
use modeler::mesh_import::{MeshUnit, read_mesh};
use modeler::viewstate::{Effect, ViewState};
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;

/// A fresh folder for this test's files.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("m7-export-ui-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The 12 triangles of a cube of side `size` with its low corner at (`x0`, 0, 0).
fn box_triangles(size: f32, x0: f32) -> Vec<[[f32; 3]; 3]> {
    let p = |x: f32, y: f32, z: f32| [x0 + x * size, y * size, z * size];
    let quads = [
        [p(0., 0., 0.), p(0., 1., 0.), p(1., 1., 0.), p(1., 0., 0.)],
        [p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)],
        [p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 1.), p(0., 0., 1.)],
        [p(1., 0., 0.), p(1., 1., 0.), p(1., 1., 1.), p(1., 0., 1.)],
        [p(1., 1., 0.), p(0., 1., 0.), p(0., 1., 1.), p(1., 1., 1.)],
        [p(0., 1., 0.), p(0., 0., 0.), p(0., 0., 1.), p(0., 1., 1.)],
    ];
    quads
        .iter()
        .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
        .collect()
}

/// A model read from an ASCII STL (in mm) with one solid per `(name, size, x0 in mm,
/// open)`; `open` leaves out one triangle, so that solid is not closed.
fn model_of(dir: &Path, solids: &[(&str, f32, f32, bool)]) -> Model {
    let mut text = String::new();
    for (name, size, x0, open) in solids {
        text.push_str(&format!("solid {name}\n"));
        let mut triangles = box_triangles(*size, *x0);
        if *open {
            triangles.pop();
        }
        for t in triangles {
            text.push_str("facet normal 0 0 0\n outer loop\n");
            for v in t {
                text.push_str(&format!("  vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            text.push_str(" endloop\nendfacet\n");
        }
        text.push_str(&format!("endsolid {name}\n"));
    }
    let path = dir.join("source.stl");
    std::fs::write(&path, text).unwrap();
    read_mesh(&path, MeshUnit::Millimeters).unwrap()
}

/// The root instance whose definition is called `name`.
fn instance_named(model: &Model, name: &str) -> InstanceId {
    model
        .root()
        .instances()
        .find(|(_, instance)| model.definition(instance.definition).unwrap().name == name)
        .map(|(id, _)| id)
        .unwrap()
}

type Corners = Vec<[[f32; 3]; 3]>;

/// (min, max) over all corners.
fn bounds(triangles: &Corners) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for corner in triangles.iter().flatten() {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(corner[axis]);
            hi[axis] = hi[axis].max(corner[axis]);
        }
    }
    (lo, hi)
}

fn read_stl(path: &Path) -> Vec<modeler_mesh::MeshObject> {
    modeler_mesh::stl::read(&std::fs::read(path).unwrap(), "part").unwrap()
}

fn read_3mf(path: &Path) -> modeler_mesh::threemf::ThreeMf {
    modeler_mesh::threemf::read(&std::fs::read(path).unwrap()).unwrap()
}

fn two_boxes(dir: &Path) -> Model {
    model_of(
        dir,
        &[("Cube", 10.0, 0.0, false), ("Block", 20.0, 50.0, false)],
    )
}

fn names(objects: &[modeler_mesh::MeshObject]) -> Vec<String> {
    let mut names: Vec<String> = objects.iter().map(|o| o.name.clone()).collect();
    names.sort();
    names
}

// ---- options and the tolerance field ----

#[test]
fn formats_have_extensions_and_labels() {
    assert_eq!(ExportFormat::ThreeMf.extension(), "3mf");
    assert_eq!(ExportFormat::Stl.extension(), "stl");
    assert_eq!(ExportFormat::ThreeMf.label(), "3MF");
    assert_eq!(ExportFormat::Stl.label(), "STL");
}

#[test]
fn default_options_are_the_whole_model_in_millimeters_as_one_file() {
    let o = ExportOptions::new(ExportFormat::Stl);
    assert_eq!(o.format, ExportFormat::Stl);
    assert!(!o.selection_only);
    assert_eq!(o.unit, MeshUnit::Millimeters);
    assert_eq!(
        o.tolerance_mm,
        modeler_mesh::print_mesh::DEFAULT_CHORD_TOLERANCE_MM
    );
    assert_eq!(o.layout, StlLayout::SingleFile);
}

#[test]
fn the_tolerance_field_takes_a_number_with_an_optional_mm() {
    assert_eq!(parse_tolerance_mm("0.01"), Some(0.01));
    assert_eq!(parse_tolerance_mm("  0.05 mm "), Some(0.05));
    assert_eq!(parse_tolerance_mm("0.2mm"), Some(0.2));
    assert_eq!(parse_tolerance_mm("1"), Some(1.0));
    assert_eq!(parse_tolerance_mm("0.001"), Some(0.001));
}

#[test]
fn the_tolerance_field_refuses_everything_else() {
    for text in [
        "",
        " ",
        "abc",
        "mm",
        "0",
        "-1",
        "0.0001",
        "1.5",
        "NaN",
        "inf",
        "1 cm",
        "0,01",
        "0.01 0.02",
    ] {
        assert_eq!(parse_tolerance_mm(text), None, "{text:?}");
    }
}

// ---- export: 3MF ----

#[test]
fn a_3mf_export_writes_every_solid_in_millimeters() {
    let dir = scratch("3mf-all");
    let model = two_boxes(&dir);
    let path = dir.join("out.3mf");
    let report = export(
        &model,
        &[],
        &ExportOptions::new(ExportFormat::ThreeMf),
        &path,
    )
    .unwrap();
    assert_eq!(report.files, vec![path.clone()]);
    assert_eq!(report.objects, 2);
    assert_eq!(report.triangles, 24);
    assert!(report.skipped.is_empty());
    let file = read_3mf(&path);
    assert!((file.unit_meters - 0.001).abs() < 1e-12);
    assert_eq!(names(&file.objects), ["Block", "Cube"]);
    let mut sizes: Vec<f32> = file
        .objects
        .iter()
        .map(|o| {
            let (lo, hi) = bounds(&o.triangles);
            hi[0] - lo[0]
        })
        .collect();
    sizes.sort_by(f32::total_cmp);
    assert!(
        (sizes[0] - 10.0).abs() < 1e-3 && (sizes[1] - 20.0).abs() < 1e-3,
        "{sizes:?}"
    );
}

#[test]
fn a_3mf_ignores_the_unit() {
    let dir = scratch("3mf-unit");
    let model = two_boxes(&dir);
    let path = dir.join("out.3mf");
    let mut options = ExportOptions::new(ExportFormat::ThreeMf);
    options.unit = MeshUnit::Inches;
    export(&model, &[], &options, &path).unwrap();
    let file = read_3mf(&path);
    assert!((file.unit_meters - 0.001).abs() < 1e-12);
    let (lo, hi) = bounds(
        &file
            .objects
            .iter()
            .flat_map(|o| o.triangles.clone())
            .collect(),
    );
    assert!((hi[0] - lo[0] - 70.0).abs() < 1e-3, "{lo:?} {hi:?}");
}

#[test]
fn the_extension_is_added_when_missing_and_kept_in_any_case() {
    let dir = scratch("extension");
    let model = two_boxes(&dir);
    let options = ExportOptions::new(ExportFormat::ThreeMf);
    let report = export(&model, &[], &options, &dir.join("plain")).unwrap();
    assert_eq!(report.files, vec![dir.join("plain.3mf")]);
    assert!(dir.join("plain.3mf").is_file() && !dir.join("plain").exists());
    let report = export(&model, &[], &options, &dir.join("LOUD.3MF")).unwrap();
    assert_eq!(report.files, vec![dir.join("LOUD.3MF")]);
    let options = ExportOptions::new(ExportFormat::Stl);
    let report = export(&model, &[], &options, &dir.join("solid.part")).unwrap();
    assert_eq!(report.files, vec![dir.join("solid.part.stl")]);
}

// ---- export: STL ----

#[test]
fn a_single_stl_holds_all_solids() {
    let dir = scratch("stl-single");
    let model = two_boxes(&dir);
    let path = dir.join("all.stl");
    let report = export(&model, &[], &ExportOptions::new(ExportFormat::Stl), &path).unwrap();
    assert_eq!(report.files, vec![path.clone()]);
    assert_eq!((report.objects, report.triangles), (2, 24));
    let objects = read_stl(&path);
    let all: Corners = objects.iter().flat_map(|o| o.triangles.clone()).collect();
    assert_eq!(all.len(), 24);
    let (lo, hi) = bounds(&all);
    assert!((hi[0] - lo[0] - 70.0).abs() < 1e-3 && (hi[1] - lo[1] - 20.0).abs() < 1e-3);
}

#[test]
fn stl_numbers_follow_the_chosen_unit() {
    let dir = scratch("stl-unit");
    let model = two_boxes(&dir);
    for (unit, width) in [
        (MeshUnit::Millimeters, 70.0),
        (MeshUnit::Centimeters, 7.0),
        (MeshUnit::Meters, 0.07),
        (MeshUnit::Inches, 70.0 / 25.4),
    ] {
        let path = dir.join(format!("{}.stl", unit.label()));
        let mut options = ExportOptions::new(ExportFormat::Stl);
        options.unit = unit;
        export(&model, &[], &options, &path).unwrap();
        let all: Corners = read_stl(&path)
            .iter()
            .flat_map(|o| o.triangles.clone())
            .collect();
        let (lo, hi) = bounds(&all);
        assert!(
            (f64::from(hi[0] - lo[0]) - width).abs() < 1e-4,
            "{unit:?}: {lo:?} {hi:?}"
        );
    }
}

#[test]
fn one_file_per_object_goes_into_a_folder() {
    let dir = scratch("stl-separate");
    let model = two_boxes(&dir);
    let folder = dir.join("parts");
    let mut options = ExportOptions::new(ExportFormat::Stl);
    options.layout = StlLayout::OneFilePerObject;
    let report = export(&model, &[], &options, &folder).unwrap();
    assert_eq!(report.files.len(), 2);
    assert_eq!((report.objects, report.triangles), (2, 24));
    assert!(folder.join("Cube.stl").is_file() && folder.join("Block.stl").is_file());
    for file in &report.files {
        assert_eq!(file.parent().unwrap(), folder);
        assert_eq!(
            read_stl(file)
                .iter()
                .map(|o| o.triangles.len())
                .sum::<usize>(),
            12
        );
    }
}

// ---- export: scope ----

#[test]
fn the_selection_limits_the_export() {
    let dir = scratch("selection");
    let model = two_boxes(&dir);
    let cube = instance_named(&model, "Cube");
    let path = dir.join("one.3mf");
    let mut options = ExportOptions::new(ExportFormat::ThreeMf);
    options.selection_only = true;
    let report = export(&model, &[Entity::Instance(cube)], &options, &path).unwrap();
    assert_eq!((report.objects, report.triangles), (1, 12));
    assert_eq!(names(&read_3mf(&path).objects), ["Cube"]);
}

#[test]
fn the_whole_model_ignores_the_selection() {
    let dir = scratch("whole");
    let model = two_boxes(&dir);
    let cube = instance_named(&model, "Cube");
    let path = dir.join("all.3mf");
    let report = export(
        &model,
        &[Entity::Instance(cube)],
        &ExportOptions::new(ExportFormat::ThreeMf),
        &path,
    )
    .unwrap();
    assert_eq!(report.objects, 2);
}

#[test]
fn a_selection_without_groups_or_components_is_refused() {
    let dir = scratch("no-selection");
    let model = two_boxes(&dir);
    let path = dir.join("none.3mf");
    let mut options = ExportOptions::new(ExportFormat::ThreeMf);
    options.selection_only = true;
    for selection in [
        vec![],
        vec![Entity::Edge(modeler_kernel::topology::EdgeId(1))],
    ] {
        let error = export(&model, &selection, &options, &path).unwrap_err();
        assert!(error.contains("Nothing selected"), "{error}");
    }
    assert!(!path.exists());
}

// ---- export: skipped and failures ----

#[test]
fn objects_that_are_not_solid_are_skipped_with_the_reason() {
    let dir = scratch("skipped");
    let model = model_of(
        &dir,
        &[("Good", 10.0, 0.0, false), ("Leaky", 10.0, 30.0, true)],
    );
    let path = dir.join("part.3mf");
    let report = export(
        &model,
        &[],
        &ExportOptions::new(ExportFormat::ThreeMf),
        &path,
    )
    .unwrap();
    assert_eq!((report.objects, report.triangles), (1, 12));
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].name, "Leaky");
    assert_eq!(names(&read_3mf(&path).objects), ["Good"]);
}

#[test]
fn when_nothing_is_solid_nothing_is_written_and_the_error_says_why() {
    let dir = scratch("none-solid");
    let model = model_of(&dir, &[("Leaky", 10.0, 0.0, true)]);
    for format in [ExportFormat::ThreeMf, ExportFormat::Stl] {
        let path = dir.join(format!("none.{}", format.extension()));
        let error = export(&model, &[], &ExportOptions::new(format), &path).unwrap_err();
        assert!(error.starts_with("Nothing to export"), "{error}");
        assert!(
            error.contains("Leaky") && error.contains("not a solid"),
            "{error}"
        );
        assert!(!path.exists());
    }
}

#[test]
fn an_empty_model_has_nothing_to_export() {
    let dir = scratch("empty");
    let path = dir.join("empty.stl");
    let error = export(
        &Model::new(),
        &[],
        &ExportOptions::new(ExportFormat::Stl),
        &path,
    )
    .unwrap_err();
    assert!(error.starts_with("Nothing to export"), "{error}");
    assert!(!path.exists());
}

#[test]
fn a_bad_tolerance_is_refused_before_anything_is_written() {
    let dir = scratch("bad-tolerance");
    let model = two_boxes(&dir);
    let path = dir.join("t.3mf");
    for tolerance in [0.0, -1.0, f64::NAN, 5.0] {
        let mut options = ExportOptions::new(ExportFormat::ThreeMf);
        options.tolerance_mm = tolerance;
        let error = export(&model, &[], &options, &path).unwrap_err();
        assert!(error.contains("tolerance"), "{error}");
    }
    assert!(!path.exists());
}

#[test]
fn a_file_that_cannot_be_written_is_an_error_naming_it() {
    let dir = scratch("unwritable");
    let model = two_boxes(&dir);
    let path = dir.join("no-such-folder").join("x.3mf");
    let error = export(
        &model,
        &[],
        &ExportOptions::new(ExportFormat::ThreeMf),
        &path,
    )
    .unwrap_err();
    assert!(error.contains("x.3mf") && !error.contains('\n'), "{error}");
}

// ---- the status line ----

fn skipped(model: &Model, dir: &Path) -> ExportReport {
    let _ = model;
    export(
        &model_of(
            dir,
            &[("Good", 10.0, 0.0, false), ("Leaky", 10.0, 30.0, true)],
        ),
        &[],
        &ExportOptions::new(ExportFormat::ThreeMf),
        &dir.join("status.3mf"),
    )
    .unwrap()
}

#[test]
fn the_status_line_counts_what_was_written() {
    let dir = scratch("status");
    let model = two_boxes(&dir);
    let options = ExportOptions::new(ExportFormat::ThreeMf);
    let report = export(&model, &[], &options, &dir.join("a.3mf")).unwrap();
    assert_eq!(
        status_message(&report, &options),
        "Exported 2 objects (24 triangles) to a.3mf"
    );
    let report = export(
        &model,
        &[Entity::Instance(instance_named(&model, "Cube"))],
        &{
            let mut o = options;
            o.selection_only = true;
            o
        },
        &dir.join("b.3mf"),
    )
    .unwrap();
    assert_eq!(
        status_message(&report, &options),
        "Exported 1 object (12 triangles) to b.3mf"
    );
}

#[test]
fn the_status_line_names_the_folder_for_one_file_per_object() {
    let dir = scratch("status-separate");
    let model = two_boxes(&dir);
    let mut options = ExportOptions::new(ExportFormat::Stl);
    options.layout = StlLayout::OneFilePerObject;
    let report = export(&model, &[], &options, &dir.join("parts")).unwrap();
    assert_eq!(
        status_message(&report, &options),
        "Exported 2 objects (24 triangles) as 2 files in parts"
    );
}

#[test]
fn the_status_line_reports_skipped_objects_and_why() {
    let dir = scratch("status-skipped");
    let report = skipped(&Model::new(), &dir);
    let message = status_message(&report, &ExportOptions::new(ExportFormat::ThreeMf));
    assert!(
        message.starts_with("Exported 1 object (12 triangles) to status.3mf; skipped Leaky "),
        "{message}"
    );
    assert!(message.contains("not a solid"), "{message}");
}

// ---- the dialog ----

struct Dialog {
    dialog: ExportDialog,
    has_selection: bool,
    outcomes: Vec<ExportOutcome>,
}

fn dialog_harness(format: ExportFormat, has_selection: bool) -> Harness<'static, Dialog> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Dialog| {
            if let Some(outcome) = export_dialog(ui, &mut state.dialog, state.has_selection) {
                state.outcomes.push(outcome);
            }
        },
        Dialog {
            dialog: ExportDialog::new(ExportOptions::new(format)),
            has_selection,
            outcomes: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn a_new_dialog_shows_the_options_it_was_given() {
    let mut options = ExportOptions::new(ExportFormat::Stl);
    options.tolerance_mm = 0.05;
    assert_eq!(ExportDialog::new(options).tolerance_text, "0.05");
    assert_eq!(ExportDialog::new(options).options, options);
    assert_eq!(
        ExportDialog::new(ExportOptions::new(ExportFormat::Stl)).tolerance_text,
        "0.01"
    );
}

#[test]
fn the_stl_dialog_has_scope_units_layout_tolerance_and_buttons() {
    let h = dialog_harness(ExportFormat::Stl, true);
    for label in [
        "Whole model",
        "Selection",
        "mm",
        "cm",
        "m",
        "in",
        "One file per object",
        "Single file",
        "Curve tolerance (mm)",
        "OK",
        "Cancel",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.state().outcomes.is_empty());
}

#[test]
fn the_3mf_dialog_has_no_units_or_layout() {
    let h = dialog_harness(ExportFormat::ThreeMf, true);
    for label in [
        "Whole model",
        "Selection",
        "Curve tolerance (mm)",
        "OK",
        "Cancel",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    for label in ["mm", "cm", "m", "in", "One file per object", "Single file"] {
        assert!(h.query_by_label(label).is_none(), "{label}");
    }
}

#[test]
fn the_scope_radios_set_the_scope() {
    let mut h = dialog_harness(ExportFormat::ThreeMf, true);
    h.get_by_label("Selection").click();
    h.run();
    assert!(h.state().dialog.options.selection_only);
    h.get_by_label("Whole model").click();
    h.run();
    assert!(!h.state().dialog.options.selection_only);
}

#[test]
fn selection_is_disabled_when_nothing_is_selected() {
    let mut h = dialog_harness(ExportFormat::ThreeMf, false);
    h.get_by_label("Selection").click();
    h.run();
    assert!(!h.state().dialog.options.selection_only);
}

#[test]
fn a_dialog_remembering_selection_falls_back_to_the_whole_model_without_one() {
    let mut options = ExportOptions::new(ExportFormat::ThreeMf);
    options.selection_only = true;
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Dialog| {
            export_dialog(ui, &mut state.dialog, state.has_selection);
        },
        Dialog {
            dialog: ExportDialog::new(options),
            has_selection: false,
            outcomes: Vec::new(),
        },
    );
    h.run();
    assert!(!h.state().dialog.options.selection_only);
}

#[test]
fn the_unit_and_layout_radios_set_them() {
    let mut h = dialog_harness(ExportFormat::Stl, false);
    h.get_by_label("in").click();
    h.run();
    h.get_by_label("One file per object").click();
    h.run();
    let options = h.state().dialog.options;
    assert_eq!(options.unit, MeshUnit::Inches);
    assert_eq!(options.layout, StlLayout::OneFilePerObject);
    h.get_by_label("Single file").click();
    h.run();
    assert_eq!(h.state().dialog.options.layout, StlLayout::SingleFile);
    assert!(h.state().outcomes.is_empty());
}

#[test]
fn ok_takes_the_tolerance_from_the_field() {
    let mut h = dialog_harness(ExportFormat::Stl, false);
    h.state_mut().dialog.tolerance_text = "0.05 mm".to_string();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![ExportOutcome::Ok]);
    assert_eq!(h.state().dialog.options.tolerance_mm, 0.05);
}

#[test]
fn ok_straight_away_keeps_the_defaults() {
    let mut h = dialog_harness(ExportFormat::ThreeMf, false);
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![ExportOutcome::Ok]);
    assert_eq!(
        h.state().dialog.options,
        ExportOptions::new(ExportFormat::ThreeMf)
    );
}

#[test]
fn ok_does_nothing_while_the_tolerance_is_not_a_number() {
    let mut h = dialog_harness(ExportFormat::Stl, false);
    h.state_mut().dialog.tolerance_text = "abc".to_string();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert!(h.state().outcomes.is_empty());
    h.state_mut().dialog.tolerance_text = "0.02".to_string();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![ExportOutcome::Ok]);
}

#[test]
fn cancel_closes_the_dialog() {
    let mut h = dialog_harness(ExportFormat::Stl, true);
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![ExportOutcome::Cancel]);
}

// ---- File > Export ----

#[test]
fn the_export_actions_open_the_dialog_and_leave_the_camera_alone() {
    for format in [ExportFormat::ThreeMf, ExportFormat::Stl] {
        let mut view = ViewState::new();
        let before = view.camera;
        assert_eq!(
            view.apply(Action::ExportMesh(format), (800.0, 600.0)),
            Effect::ExportDialog(format)
        );
        assert_eq!(view.camera, before);
    }
}

fn file_menu(can_export: bool) -> (Harness<'static>, Rc<RefCell<Vec<Action>>>) {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        can_export,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("File").click();
    h.run();
    (h, seen)
}

#[test]
fn the_file_menu_has_both_exports_between_import_and_save() {
    let (mut h, seen) = file_menu(true);
    let import = h.get_by_label("Import...").rect().top();
    let threemf = h.get_by_label("Export 3MF...").rect().top();
    let stl = h.get_by_label("Export STL...").rect().top();
    let save = h.get_by_label("Save").rect().top();
    assert!(import < threemf && threemf < stl && stl < save);
    h.get_by_label("Export STL...").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::ExportMesh(ExportFormat::Stl)]);
    assert!(h.query_by_label("Export STL...").is_none());
}

#[test]
fn the_3mf_item_gives_the_3mf_action() {
    let (mut h, seen) = file_menu(true);
    h.get_by_label("Export 3MF...").click();
    h.run();
    assert_eq!(
        *seen.borrow(),
        vec![Action::ExportMesh(ExportFormat::ThreeMf)]
    );
}

#[test]
fn the_exports_are_disabled_without_anything_to_export() {
    for label in ["Export 3MF...", "Export STL..."] {
        let (mut h, seen) = file_menu(false);
        h.get_by_label(label).click();
        h.run();
        assert!(seen.borrow().is_empty(), "{label}");
    }
}
