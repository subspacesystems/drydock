// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-import-ui: STL and 3MF files through Open and Import, and the units choice.
//! See docs/specs/m7-import-ui.md.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use drydock::chrome::{MenuState, menu_bar};
use drydock::components_panel::read_component_source;
use drydock::document::{Document, FileKind};
use drydock::editor::Editor;
use drydock::input::{Action, EditKey};
use drydock::mesh_import::*;
use drydock::viewstate::{Effect, ViewState};
use drydock_kernel::copy::extract;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A fresh folder for this test's files.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("m7-import-ui-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The 12 triangles of an axis-aligned box from the origin, corners counter-clockwise seen
/// from outside.
fn box_triangles(w: f32, d: f32, h: f32) -> Vec<[[f32; 3]; 3]> {
    let p = |x: f32, y: f32, z: f32| [x * w, y * d, z * h];
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

/// An ASCII STL file of a `size` box, named `name.stl` in `dir`.
fn write_stl(dir: &Path, name: &str, size: f32) -> PathBuf {
    let mut text = String::from("solid part\n");
    for t in box_triangles(size, size, size) {
        text.push_str("facet normal 0 0 0\n outer loop\n");
        for v in t {
            text.push_str(&format!("  vertex {} {} {}\n", v[0], v[1], v[2]));
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

/// A stored (uncompressed) 3MF archive with `objects` boxes of size 10 in the given unit
/// attribute, named `name.3mf` in `dir`. Object `i` is called `Part i` and is shifted
/// along x by `20 * i`.
fn write_3mf(dir: &Path, name: &str, unit: &str, objects: usize) -> PathBuf {
    let mut resources = String::new();
    let mut build = String::new();
    for i in 0..objects {
        let mut vertices = String::new();
        let mut triangles = String::new();
        let tris = box_triangles(10.0, 10.0, 10.0);
        for (n, t) in tris.iter().enumerate() {
            for v in t {
                vertices.push_str(&format!(
                    "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
                    v[0] + 20.0 * i as f32,
                    v[1],
                    v[2]
                ));
            }
            triangles.push_str(&format!(
                "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                3 * n,
                3 * n + 1,
                3 * n + 2
            ));
        }
        resources.push_str(&format!(
            "<object id=\"{}\" type=\"model\" name=\"Part {i}\"><mesh><vertices>{vertices}</vertices>\
             <triangles>{triangles}</triangles></mesh></object>",
            i + 1
        ));
        build.push_str(&format!("<item objectid=\"{}\"/>", i + 1));
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"{unit}\" \
         xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\
         <resources>{resources}</resources><build>{build}</build></model>"
    );
    let entry = "3D/3dmodel.model";
    let data = xml.as_bytes();
    let mut out = Vec::new();
    let mut central = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
    le16(&mut out, 20);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le32(&mut out, 0);
    le32(&mut out, 0);
    le32(&mut out, data.len() as u32);
    le32(&mut out, data.len() as u32);
    le16(&mut out, entry.len() as u16);
    le16(&mut out, 0);
    out.extend_from_slice(entry.as_bytes());
    out.extend_from_slice(data);
    central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
    le16(&mut central, 20);
    le16(&mut central, 20);
    le16(&mut central, 0);
    le16(&mut central, 0);
    le32(&mut central, 0);
    le32(&mut central, 0);
    le32(&mut central, data.len() as u32);
    le32(&mut central, data.len() as u32);
    le16(&mut central, entry.len() as u16);
    le16(&mut central, 0);
    le16(&mut central, 0);
    le16(&mut central, 0);
    le16(&mut central, 0);
    le32(&mut central, 0);
    le32(&mut central, 0);
    central.extend_from_slice(entry.as_bytes());
    let start = out.len() as u32;
    let len = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, 1);
    le16(&mut out, 1);
    le32(&mut out, len);
    le32(&mut out, start);
    le16(&mut out, 0);
    let path = dir.join(name);
    std::fs::write(&path, out).unwrap();
    path
}

fn root_instances(model: &Model) -> Vec<InstanceId> {
    model.root().instances().map(|(id, _)| id).collect()
}

/// World-space bounds of everything in the root, in meters.
fn world_bounds(model: &Model) -> ([f64; 3], [f64; 3]) {
    let selection: Vec<Entity> = root_instances(model)
        .into_iter()
        .map(Entity::Instance)
        .collect();
    let clipboard = extract(model, BodyId::Root, &selection).unwrap();
    let (min, max) = clipboard.bounds().unwrap();
    (min.to_array(), max.to_array())
}

fn size_x(model: &Model) -> f64 {
    let (min, max) = world_bounds(model);
    max[0] - min[0]
}

fn group_names(model: &Model) -> Vec<String> {
    let mut names: Vec<String> = model
        .definitions()
        .filter(|(_, info)| info.group)
        .map(|(_, info)| info.name.clone())
        .collect();
    names.sort();
    names
}

// ---- file kinds and units ----

#[test]
fn format_comes_from_the_extension_in_any_case() {
    for (name, format) in [
        ("a.stl", Some(MeshFormat::Stl)),
        ("a.STL", Some(MeshFormat::Stl)),
        ("dir/a.b.Stl", Some(MeshFormat::Stl)),
        ("a.3mf", Some(MeshFormat::ThreeMf)),
        ("a.3MF", Some(MeshFormat::ThreeMf)),
        ("a.dae", None),
        ("a.modeler", None),
        ("a.stl.txt", None),
        ("stl", None),
        ("", None),
    ] {
        assert_eq!(mesh_format(Path::new(name)), format, "{name}");
    }
}

#[test]
fn only_stl_asks_for_units() {
    assert!(needs_units(Path::new("a.stl")));
    assert!(needs_units(Path::new("A.STL")));
    assert!(!needs_units(Path::new("a.3mf")));
    assert!(!needs_units(Path::new("a.3MF")));
    assert!(!needs_units(Path::new("a.dae")));
    assert!(!needs_units(Path::new("a.modeler")));
}

#[test]
fn units_are_mm_cm_m_in_and_mm_is_the_default() {
    assert_eq!(MeshUnit::default(), MeshUnit::Millimeters);
    assert_eq!(
        MeshUnit::ALL,
        [
            MeshUnit::Millimeters,
            MeshUnit::Centimeters,
            MeshUnit::Meters,
            MeshUnit::Inches
        ]
    );
    let labels: Vec<&str> = MeshUnit::ALL.iter().map(|u| u.label()).collect();
    assert_eq!(labels, ["mm", "cm", "m", "in"]);
    let meters: Vec<f64> = MeshUnit::ALL.iter().map(|u| u.meters()).collect();
    assert_eq!(meters, [0.001, 0.01, 1.0, 0.0254]);
}

// ---- reading ----

#[test]
fn an_stl_is_scaled_by_the_chosen_unit() {
    let dir = scratch("stl-units");
    let path = write_stl(&dir, "cube.stl", 10.0);
    for (unit, expected) in [
        (MeshUnit::Millimeters, 0.01),
        (MeshUnit::Centimeters, 0.1),
        (MeshUnit::Meters, 10.0),
        (MeshUnit::Inches, 0.254),
    ] {
        let model = read_mesh(&path, unit).unwrap();
        assert!((size_x(&model) - expected).abs() < 1e-6, "{unit:?}");
    }
}

#[test]
fn an_stl_becomes_one_group_named_after_the_file() {
    let dir = scratch("stl-group");
    let path = write_stl(&dir, "cube.stl", 10.0);
    let model = read_mesh(&path, MeshUnit::default()).unwrap();
    assert_eq!(root_instances(&model).len(), 1);
    assert_eq!(group_names(&model), vec!["part".to_string()]);
}

#[test]
fn a_3mf_keeps_its_own_unit_whatever_unit_is_passed() {
    let dir = scratch("3mf-units");
    let mm = write_3mf(&dir, "mm.3mf", "millimeter", 1);
    let inch = write_3mf(&dir, "in.3mf", "inch", 1);
    for unit in MeshUnit::ALL {
        assert!((size_x(&read_mesh(&mm, unit).unwrap()) - 0.01).abs() < 1e-6);
        assert!((size_x(&read_mesh(&inch, unit).unwrap()) - 0.254).abs() < 1e-6);
    }
}

#[test]
fn a_3mf_becomes_one_group_per_object() {
    let dir = scratch("3mf-groups");
    let path = write_3mf(&dir, "two.3mf", "millimeter", 2);
    let model = read_mesh(&path, MeshUnit::default()).unwrap();
    assert_eq!(root_instances(&model).len(), 2);
    assert_eq!(
        group_names(&model),
        vec!["Part 0".to_string(), "Part 1".to_string()]
    );
    // Two 10 mm boxes 20 mm apart.
    assert!((size_x(&model) - 0.03).abs() < 1e-6);
}

#[test]
fn read_errors_are_one_line_and_name_the_file() {
    let dir = scratch("errors");
    let garbage = dir.join("garbage.stl");
    std::fs::write(&garbage, "this is not a mesh").unwrap();
    let bad_zip = dir.join("broken.3mf");
    std::fs::write(&bad_zip, "PK nope").unwrap();
    let other = dir.join("drawing.dwg");
    std::fs::write(&other, "x").unwrap();
    for (path, name) in [
        (garbage, "garbage.stl"),
        (bad_zip, "broken.3mf"),
        (other, "drawing.dwg"),
        (dir.join("missing.stl"), "missing.stl"),
        (dir.join("missing.3mf"), "missing.3mf"),
    ] {
        let err = read_mesh(&path, MeshUnit::default()).unwrap_err();
        assert!(err.contains(name), "{err}");
        assert!(!err.contains('\n'), "{err}");
    }
}

// ---- Open: Document ----

#[test]
fn open_mesh_makes_an_unmodified_document_that_saves_under_a_new_name() {
    let dir = scratch("doc");
    let path = write_stl(&dir, "cube.stl", 10.0);
    let document = Document::open_mesh(&path, MeshUnit::Inches).unwrap();
    assert_eq!(document.path, path);
    assert_eq!(document.kind, FileKind::Dae);
    assert!(!document.untitled);
    assert!(!document.modified);
    assert!(document.kernel_error.is_none());
    let model = document.kernel.as_ref().unwrap();
    assert_eq!(root_instances(model).len(), 1);
    assert!((size_x(model) - 0.254).abs() < 1e-6);
    assert!(
        document.stats.triangles >= 12,
        "{}",
        document.stats.triangles
    );
    assert!(!document.scene.batches.is_empty());
}

#[test]
fn open_mesh_reads_3mf_too() {
    let dir = scratch("doc3mf");
    let path = write_3mf(&dir, "two.3mf", "millimeter", 2);
    let document = Document::open_mesh(&path, MeshUnit::Inches).unwrap();
    assert_eq!(root_instances(document.kernel.as_ref().unwrap()).len(), 2);
}

#[test]
fn open_mesh_errors_name_the_file() {
    let dir = scratch("doc-err");
    let path = dir.join("garbage.stl");
    std::fs::write(&path, "nope").unwrap();
    let err = Document::open_mesh(&path, MeshUnit::default())
        .err()
        .unwrap();
    assert!(err.contains("garbage.stl"), "{err}");
    assert!(!err.contains('\n'), "{err}");
}

#[test]
fn open_sends_stl_and_3mf_files_through_open_mesh_in_millimeters() {
    let dir = scratch("doc-open");
    let stl = write_stl(&dir, "cube.stl", 10.0);
    let threemf = write_3mf(&dir, "one.3mf", "inch", 1);
    let a = Document::open(&stl).unwrap();
    assert_eq!(a.kind, FileKind::Dae);
    assert!((size_x(a.kernel.as_ref().unwrap()) - 0.01).abs() < 1e-6);
    let b = Document::open(&threemf).unwrap();
    assert!((size_x(b.kernel.as_ref().unwrap()) - 0.254).abs() < 1e-6);
}

#[test]
fn a_mesh_file_can_be_imported_as_a_component() {
    let dir = scratch("component");
    let path = write_stl(&dir, "cube.stl", 10.0);
    let (model, name) = read_component_source(&path).unwrap();
    assert_eq!(name, "cube");
    assert_eq!(root_instances(&model).len(), 1);
}

// ---- Import: Editor ----

#[test]
fn import_groups_adds_the_groups_in_one_undo_step() {
    let dir = scratch("editor");
    let source = read_mesh(
        &write_3mf(&dir, "two.3mf", "millimeter", 2),
        MeshUnit::default(),
    )
    .unwrap();
    let mut editor = Editor::new();
    let mut model = Model::new();
    let added = editor.import_groups(&mut model, &source).unwrap();
    assert_eq!(added, 2);
    assert_eq!(root_instances(&model).len(), 2);
    assert_eq!(
        group_names(&model),
        vec!["Part 0".to_string(), "Part 1".to_string()]
    );
    assert!((size_x(&model) - 0.03).abs() < 1e-6);
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Import Mesh"));
    assert!(model.validate().is_empty());
    editor.key(EditKey::Undo, &mut model);
    assert!(root_instances(&model).is_empty());
    assert!(group_names(&model).is_empty());
}

#[test]
fn import_groups_keeps_what_the_model_already_holds() {
    let dir = scratch("editor-keep");
    let source = read_mesh(&write_stl(&dir, "cube.stl", 10.0), MeshUnit::default()).unwrap();
    let mut editor = Editor::new();
    let mut model = Model::new();
    editor.import_groups(&mut model, &source).unwrap();
    editor.import_groups(&mut model, &source).unwrap();
    assert_eq!(root_instances(&model).len(), 2);
    assert_eq!(group_names(&model).len(), 2, "names stay unique");
    assert_eq!(editor.undo_stack().undo_count(), 2);
    editor.key(EditKey::Undo, &mut model);
    assert_eq!(root_instances(&model).len(), 1);
}

#[test]
fn import_groups_of_an_empty_source_is_refused_and_changes_nothing() {
    let mut editor = Editor::new();
    let mut model = Model::new();
    let err = editor.import_groups(&mut model, &Model::new()).unwrap_err();
    assert!(err.contains("Nothing to import"), "{err}");
    assert!(!err.contains('\n'));
    assert_eq!(editor.message(), Some(err.as_str()));
    assert!(root_instances(&model).is_empty());
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

// ---- the units choice (driven with egui_kittest) ----

struct Choice {
    unit: MeshUnit,
    outcomes: Vec<UnitsOutcome>,
}

fn choice_harness(unit: MeshUnit) -> Harness<'static, Choice> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Choice| {
            if let Some(outcome) = units_choice(ui, "cube.stl", &mut state.unit) {
                state.outcomes.push(outcome);
            }
        },
        Choice {
            unit,
            outcomes: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn the_units_choice_lists_the_units_with_ok_and_cancel() {
    let h = choice_harness(MeshUnit::default());
    for label in ["mm", "cm", "m", "in", "OK", "Cancel"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label_contains("cube.stl").is_some());
    assert!(h.state().outcomes.is_empty());
}

#[test]
fn ok_straight_away_keeps_millimeters() {
    let mut h = choice_harness(MeshUnit::default());
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![UnitsOutcome::Ok]);
    assert_eq!(h.state().unit, MeshUnit::Millimeters);
}

#[test]
fn picking_a_unit_then_ok_returns_it() {
    for unit in MeshUnit::ALL {
        let mut h = choice_harness(MeshUnit::default());
        h.get_by_label(unit.label()).click();
        h.run();
        assert!(h.state().outcomes.is_empty(), "picking does not close it");
        assert_eq!(h.state().unit, unit);
        h.get_by_label("OK").click();
        h.run();
        assert_eq!(h.state().outcomes, vec![UnitsOutcome::Ok]);
        assert_eq!(h.state().unit, unit);
    }
}

#[test]
fn the_unit_passed_in_starts_selected() {
    let mut h = choice_harness(MeshUnit::Inches);
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().unit, MeshUnit::Inches);
}

#[test]
fn cancel_closes_the_choice() {
    let mut h = choice_harness(MeshUnit::default());
    h.get_by_label("cm").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![UnitsOutcome::Cancel]);
}

// ---- File > Import... ----

#[test]
fn import_is_an_action_for_the_import_dialog() {
    let mut view = ViewState::new();
    let before = view.camera;
    assert_eq!(
        view.apply(Action::Import, (800.0, 600.0)),
        Effect::ImportDialog
    );
    assert_eq!(view.camera, before);
}

#[test]
fn the_file_menu_has_import_after_open() {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("File").click();
    h.run();
    let open = h.get_by_label("Open...").rect().top();
    let import = h.get_by_label("Import...").rect().top();
    let save = h.get_by_label("Save").rect().top();
    assert!(open < import && import < save, "{open} {import} {save}");
    h.get_by_label("Import...").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::Import]);
    // The menu closed.
    assert!(h.query_by_label("Import...").is_none());
}
