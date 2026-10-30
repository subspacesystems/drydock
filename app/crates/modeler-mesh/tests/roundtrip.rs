// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Print round trip (`docs/specs/m7-roundtrip.md`): solids exported to STL and 3MF come back
//! with the same volume and bounding box, and deliberately broken models are found by the
//! solid check, fixed by repair and then export like the good ones.

use std::path::{Path, PathBuf};

use glam::{DMat4, DQuat, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::repair::repair_body;
use modeler_kernel::solid::check_body;
use modeler_mesh::print_mesh::*;
use modeler_mesh::validate::validate;
use modeler_mesh::{import_3mf, import_stl, stl_write, threemf_write};

mod print_common;
use print_common::*;

const TOL_MM: f64 = 0.01;
/// Welding distance for the validator, millimeters: far below any feature, far above the
/// rounding of a number to f32.
const WELD_MM: f64 = 1e-6;

/// A scratch folder that goes away with the test.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("modeler-roundtrip-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn export(model: &Model) -> PrintMesh {
    print_mesh(
        model,
        PrintScope::Model,
        &PrintOptions {
            chord_tolerance_mm: TOL_MM,
        },
    )
}

fn all_triangles(objects: &[PrintObject]) -> Vec<Triangle> {
    objects.iter().flat_map(|o| o.triangles.clone()).collect()
}

/// The objects of a re-imported model, exported again (the same path a user takes to print
/// an imported file).
fn objects_of(model: &Model) -> Vec<PrintObject> {
    let out = export(model);
    assert_eq!(out.skipped, vec![], "a re-imported solid was left out");
    out.objects
}

/// `back` has the same volume (within 1e-6 relative) and bounding box (within 1e-6 of the
/// largest coordinate, which is what f32 can hold) as `original`, and both are sound meshes.
#[track_caller]
fn assert_same_solid(label: &str, original: &[Triangle], back: &[Triangle]) {
    let a = validate(original, WELD_MM);
    let b = validate(back, WELD_MM);
    assert!(a.is_valid(), "{label}: exported mesh is not sound: {a:?}");
    assert!(
        b.is_valid(),
        "{label}: re-imported mesh is not sound: {b:?}"
    );
    assert!(
        (a.volume - b.volume).abs() <= 1e-6 * a.volume,
        "{label}: volume {} became {}",
        a.volume,
        b.volume
    );
    let ((alo, ahi), (blo, bhi)) = (bounds(original), bounds(back));
    let reach = alo.abs().max(ahi.abs()).max_element().max(1.0);
    let tol = 1e-6 * reach;
    assert!(
        alo.distance(blo) <= tol && ahi.distance(bhi) <= tol,
        "{label}: bounds {alo:?}..{ahi:?} became {blo:?}..{bhi:?}"
    );
}

#[track_caller]
fn roundtrip_stl_separate(label: &str, objects: &[PrintObject]) {
    let dir = Scratch::new(&format!("sep-{}", label.replace(' ', "_")));
    let files = stl_write::write_separate(objects, dir.path()).unwrap();
    assert_eq!(files.len(), objects.len(), "{label}: one file per object");
    let mut back = Vec::new();
    for file in &files {
        assert_eq!(file.extension().and_then(|e| e.to_str()), Some("stl"));
        let model = import_stl(file, 0.001).unwrap();
        let objs = objects_of(&model);
        assert_eq!(objs.len(), 1, "{label}: a file is one solid");
        back.extend(objs);
    }
    assert_same_solid(label, &all_triangles(objects), &all_triangles(&back));
}

#[track_caller]
fn roundtrip_stl_combined(label: &str, objects: &[PrintObject]) {
    let dir = Scratch::new(&format!("one-{}", label.replace(' ', "_")));
    let file = dir.path().join("all.stl");
    stl_write::write_combined(objects, &file).unwrap();
    let back = objects_of(&import_stl(&file, 0.001).unwrap());
    assert_same_solid(label, &all_triangles(objects), &all_triangles(&back));
}

#[track_caller]
fn roundtrip_3mf(label: &str, objects: &[PrintObject]) {
    let dir = Scratch::new(&format!("3mf-{}", label.replace(' ', "_")));
    let file = dir.path().join("all.3mf");
    threemf_write::write(objects, &file).unwrap();
    let back = objects_of(&import_3mf(&file).unwrap());
    assert_eq!(back.len(), objects.len(), "{label}: one object each");
    let mut names: Vec<&str> = objects.iter().map(|o| o.name.as_str()).collect();
    let mut back_names: Vec<&str> = back.iter().map(|o| o.name.as_str()).collect();
    names.sort_unstable();
    back_names.sort_unstable();
    assert_eq!(names, back_names, "{label}: names");
    assert_same_solid(label, &all_triangles(objects), &all_triangles(&back));
}

#[track_caller]
fn roundtrip_everything(label: &str, model: &Model) {
    let out = export(model);
    assert_eq!(out.skipped, vec![], "{label}: nothing should be left out");
    assert!(!out.objects.is_empty(), "{label}: nothing exported");
    roundtrip_stl_separate(label, &out.objects);
    roundtrip_stl_combined(label, &out.objects);
    roundtrip_3mf(label, &out.objects);
}

// --- closed solids -------------------------------------------------------------------------

/// Closed solids, each as its own model, with what they are for in the label.
fn solids() -> Vec<(&'static str, Model)> {
    let mut out = Vec::new();

    let mut m = Model::new();
    let (def, _, _) = box_group(&mut m, "Block", v(0.0, 0.0, 0.0), v(0.02, 0.01, 0.005));
    place(&mut m, BodyId::Root, def, DMat4::IDENTITY, "Block");
    out.push(("box", m));

    let mut m = Model::new();
    let (def, _) = plate_with_hole(&mut m, "Plate");
    place(&mut m, BodyId::Root, def, DMat4::IDENTITY, "Plate");
    out.push(("plate with a hole", m));

    let mut m = Model::new();
    let def = cylinder_group(&mut m, "Prism", 0.01, 0.02, 12, false);
    place(&mut m, BodyId::Root, def, DMat4::IDENTITY, "Prism");
    out.push(("faceted cylinder", m));

    let mut m = Model::new();
    let def = cylinder_group(&mut m, "Round", 0.01, 0.02, 24, true);
    place(&mut m, BodyId::Root, def, DMat4::IDENTITY, "Round");
    out.push(("cylinder with circles", m));

    let mut m = Model::new();
    let (def, _, _) = box_group(&mut m, "Tilted", v(0.0, 0.0, 0.0), v(0.03, 0.02, 0.01));
    place(
        &mut m,
        BodyId::Root,
        def,
        DMat4::from_rotation_translation(DQuat::from_rotation_z(0.5), v(0.1, 0.2, 0.3)),
        "Tilted",
    );
    out.push(("rotated and moved", m));

    let mut m = Model::new();
    let (block, _, _) = box_group(&mut m, "Block", v(0.0, 0.0, 0.0), v(0.01, 0.02, 0.03));
    let round = cylinder_group(&mut m, "Round", 0.005, 0.04, 16, true);
    place(&mut m, BodyId::Root, block, DMat4::IDENTITY, "A");
    place(
        &mut m,
        BodyId::Root,
        round,
        DMat4::from_translation(v(0.05, 0.0, 0.0)),
        "B",
    );
    place(
        &mut m,
        BodyId::Root,
        block,
        DMat4::from_translation(v(0.0, 0.1, 0.0)),
        "C",
    );
    out.push(("three parts, two sharing a component", m));

    out
}

#[test]
fn closed_solids_survive_a_binary_stl_file_per_object() {
    for (label, model) in solids() {
        let out = export(&model);
        assert_eq!(out.skipped, vec![], "{label}");
        roundtrip_stl_separate(label, &out.objects);
    }
}

#[test]
fn closed_solids_survive_one_combined_stl_file() {
    for (label, model) in solids() {
        let out = export(&model);
        assert_eq!(out.skipped, vec![], "{label}");
        roundtrip_stl_combined(label, &out.objects);
    }
}

#[test]
fn closed_solids_survive_a_3mf_file() {
    for (label, model) in solids() {
        let out = export(&model);
        assert_eq!(out.skipped, vec![], "{label}");
        roundtrip_3mf(label, &out.objects);
    }
}

#[test]
fn the_exported_mesh_of_every_solid_is_a_sound_mesh_of_the_kernel_volume() {
    for (label, model) in solids() {
        let out = export(&model);
        let r = validate(&all_triangles(&out.objects), WELD_MM);
        assert!(r.is_valid(), "{label}: {r:?}");
        let mut expected = 0.0;
        for (_, instance) in model.root().instances() {
            let def = BodyId::Definition(instance.definition);
            let scale = instance.transform.determinant().abs();
            expected += check_body(&model, def).unwrap().volume.unwrap() * scale * 1e9;
        }
        // Circles are re-segmented finer than the kernel's polygon, so they differ a little.
        let rel = if label.contains("circles") || label.contains("three parts") {
            0.02
        } else {
            1e-9
        };
        assert!(
            (r.volume - expected).abs() <= rel * expected,
            "{label}: {} != {expected}",
            r.volume
        );
    }
}

#[test]
fn the_curved_cylinder_has_more_triangles_than_the_faceted_one() {
    let all = solids();
    let count = |label: &str| {
        let (_, model) = all.iter().find(|(l, _)| *l == label).unwrap();
        all_triangles(&export(model).objects).len()
    };
    assert!(count("cylinder with circles") > 4 * count("faceted cylinder"));
}

// --- broken models ---------------------------------------------------------------------------

/// A broken group and the good one it should become: their root models and the broken
/// group's body.
struct Case {
    broken: Model,
    body: BodyId,
    good: Model,
}

fn good_box(size: DVec3) -> Model {
    let mut model = Model::new();
    let (def, _, _) = box_group(&mut model, "Part", v(0.0, 0.0, 0.0), size);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Part");
    model
}

/// A box with one face wound the wrong way.
fn reversed_face() -> Case {
    let mut model = Model::new();
    let (def, body, _) =
        box_group_turned(&mut model, "Part", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0), &[2]);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Part");
    Case {
        broken: model,
        body,
        good: good_box(v(1.0, 1.0, 1.0)),
    }
}

/// Two 1 m cubes side by side with the wall between them still there.
fn internal_wall() -> Case {
    let mut model = Model::new();
    let (def, body) = definition(&mut model, "Part", true);
    let mut points = Vec::new();
    for k in 0..2 {
        for j in 0..2 {
            for i in 0..3 {
                points.push(v(f64::from(i), f64::from(j), f64::from(k)));
            }
        }
    }
    let at = |i: usize, j: usize, k: usize| i + 3 * j + 6 * k;
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for i in 0..2 {
        faces.push(vec![
            at(i, 0, 0),
            at(i, 1, 0),
            at(i + 1, 1, 0),
            at(i + 1, 0, 0),
        ]);
        faces.push(vec![
            at(i, 0, 1),
            at(i + 1, 0, 1),
            at(i + 1, 1, 1),
            at(i, 1, 1),
        ]);
        faces.push(vec![
            at(i, 0, 0),
            at(i + 1, 0, 0),
            at(i + 1, 0, 1),
            at(i, 0, 1),
        ]);
        faces.push(vec![
            at(i, 1, 0),
            at(i, 1, 1),
            at(i + 1, 1, 1),
            at(i + 1, 1, 0),
        ]);
    }
    faces.push(vec![at(0, 0, 0), at(0, 0, 1), at(0, 1, 1), at(0, 1, 0)]);
    faces.push(vec![at(2, 0, 0), at(2, 1, 0), at(2, 1, 1), at(2, 0, 1)]);
    faces.push(vec![at(1, 0, 0), at(1, 0, 1), at(1, 1, 1), at(1, 1, 0)]);
    let specs: Vec<FaceSpec> = faces.iter().map(|f| (&f[..], &[][..])).collect();
    build(&mut model, body, &points, &specs, &[]);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Part");
    Case {
        broken: model,
        body,
        good: good_box(v(2.0, 1.0, 1.0)),
    }
}

/// A 1 m cube without its top face: one flat loop of four open edges.
fn flat_hole() -> Case {
    let mut model = Model::new();
    let def = open_box_group(&mut model, "Part");
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Part");
    Case {
        broken: model,
        body: BodyId::Definition(def),
        good: good_box(v(1.0, 1.0, 1.0)),
    }
}

/// A closed 1 m cube with one stray edge hanging in space.
fn loose_edge() -> Case {
    let mut model = Model::new();
    let (def, body, _) = box_group(&mut model, "Part", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Part");
    model
        .edit_geometry(body, &mut ModelEdit::new(), |ctx, log| {
            let a = ctx.insert_vertex(v(2.0, 0.0, 0.0), log);
            let b = ctx.insert_vertex(v(3.0, 0.0, 0.0), log);
            ctx.insert_edge(a, b, log).unwrap();
        })
        .unwrap();
    Case {
        broken: model,
        body,
        good: good_box(v(1.0, 1.0, 1.0)),
    }
}

/// Finds the problem, repairs it in one undo step, and checks the result prints like the
/// good model: same volume and box, through every file format.
#[track_caller]
fn repair_and_export(label: &str, mut case: Case) {
    let before = check_body(&case.broken, case.body).unwrap();
    assert!(!before.is_clean(), "{label}: should start out unclean");

    let mut undo = UndoStack::new();
    let report = repair_body(&mut case.broken, case.body, &mut undo, "Repair Solid").unwrap();
    assert!(report.changed(), "{label}: repair changed nothing");
    assert_eq!(report.unfixed, vec![], "{label}");
    assert_eq!(undo.undo_count(), 1, "{label}: one undo step");
    assert_eq!(case.broken.validate(), vec![], "{label}");

    let after = check_body(&case.broken, case.body).unwrap();
    assert!(after.is_clean(), "{label}: still unclean: {after:?}");
    let good = check_body(&case.good, BodyId::Definition(only_definition(&case.good))).unwrap();
    let (got, want) = (after.volume.unwrap(), good.volume.unwrap());
    assert!(
        (got - want).abs() <= 1e-9 * want,
        "{label}: volume {got} != {want}"
    );

    let printed = export(&case.broken);
    assert_eq!(printed.skipped, vec![], "{label}");
    assert_eq!(printed.objects.len(), 1, "{label}");
    assert_same_solid(
        &format!("{label} repaired vs good"),
        &all_triangles(&export(&case.good).objects),
        &all_triangles(&printed.objects),
    );
    roundtrip_everything(label, &case.broken);

    // Nothing left to do: a second repair is a no-op with no undo step.
    let again = repair_body(&mut case.broken, case.body, &mut undo, "Repair Solid").unwrap();
    assert!(!again.changed(), "{label}");
    assert_eq!(undo.undo_count(), 1, "{label}");
}

fn only_definition(model: &Model) -> DefinitionId {
    model.root().instances().next().unwrap().1.definition
}

#[test]
fn a_reversed_face_is_found_and_repaired_then_exports() {
    let case = reversed_face();
    let before = check_body(&case.broken, case.body).unwrap();
    assert!(before.is_solid());
    assert_eq!(before.reversed_faces.len(), 1);
    repair_and_export("reversed face", case);
}

#[test]
fn an_internal_wall_is_found_and_removed_then_exports() {
    let case = internal_wall();
    let before = check_body(&case.broken, case.body).unwrap();
    assert!(!before.is_solid());
    assert_eq!(before.internal_faces.len(), 1);
    assert_eq!(before.shared_edges.len(), 4);
    let skipped = export(&case.broken).skipped;
    assert_eq!(skipped.len(), 1, "an unrepaired model is not exported");
    repair_and_export("internal wall", case);
}

#[test]
fn a_flat_hole_is_found_and_filled_then_exports() {
    let case = flat_hole();
    let before = check_body(&case.broken, case.body).unwrap();
    assert!(!before.is_solid());
    assert_eq!(before.open_edges.len(), 4);
    let skipped = export(&case.broken).skipped;
    assert_eq!(skipped.len(), 1, "an unrepaired model is not exported");
    repair_and_export("flat hole", case);
}

#[test]
fn a_loose_edge_is_found_and_removed_then_exports() {
    let case = loose_edge();
    let before = check_body(&case.broken, case.body).unwrap();
    assert!(!before.is_solid());
    assert_eq!(before.loose_edges.len(), 1);
    let skipped = export(&case.broken).skipped;
    assert_eq!(skipped.len(), 1, "an unrepaired model is not exported");
    repair_and_export("loose edge", case);
}

#[test]
fn a_reversed_face_exports_turned_even_before_repair() {
    let case = reversed_face();
    let out = export(&case.broken);
    assert_eq!(out.skipped, vec![]);
    let r = validate(&all_triangles(&out.objects), WELD_MM);
    assert!(r.is_valid(), "{r:?}");
    roundtrip_everything("reversed face, unrepaired", &case.broken);
}
