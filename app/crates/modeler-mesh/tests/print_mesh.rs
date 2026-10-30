// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Print mesh (`docs/specs/m7-print-mesh.md`).

use std::f64::consts::{FRAC_PI_2, PI};

use glam::{DMat4, DQuat, DVec3};
use modeler_kernel::curve::MIN_CHORD_TOL;
use modeler_kernel::model::*;
use modeler_kernel::solid::check_body;
use modeler_mesh::print_mesh::*;

mod print_common;
use print_common::*;

fn export(model: &Model, tol_mm: f64) -> PrintMesh {
    print_mesh(
        model,
        PrintScope::Model,
        &PrintOptions {
            chord_tolerance_mm: tol_mm,
        },
    )
}

fn unit_cube(model: &mut Model, name: &str, transform: DMat4) -> InstanceId {
    let (def, _, _) = box_group(model, name, v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    place(model, BodyId::Root, def, transform, name)
}

/// Every triangle's normal points away from `centre` (convex solids).
#[track_caller]
fn assert_outward(triangles: &[Triangle], centre: DVec3) {
    for t in triangles {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        let mid = (t[0] + t[1] + t[2]) / 3.0;
        assert!(n.dot(mid - centre) > 0.0, "triangle faces inward: {t:?}");
    }
}

// --- units, volume, winding -------------------------------------------------------------

#[test]
fn unit_cube_is_a_thousand_millimeters() {
    let mut model = Model::new();
    unit_cube(&mut model, "Cube", DMat4::IDENTITY);
    let out = export(&model, 0.01);
    assert!(out.skipped.is_empty());
    assert_eq!(out.objects.len(), 1);
    let o = &out.objects[0];
    assert_eq!(o.name, "Cube");
    assert_eq!(o.triangles.len(), 12);
    let (lo, hi) = bounds(&o.triangles);
    assert_close(lo, v(0.0, 0.0, 0.0), 1e-9);
    assert_close(hi, v(1000.0, 1000.0, 1000.0), 1e-9);
    assert_rel(volume(&o.triangles), 1e9, 1e-9);
    assert_outward(&o.triangles, v(500.0, 500.0, 500.0));
    assert_closed(&o.triangles);
}

#[test]
fn mesh_volume_equals_the_solid_report_volume() {
    let mut model = Model::new();
    let (def, body) = plate_with_hole(&mut model, "Plate");
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Plate");
    let (def, body2, _) = box_group(&mut model, "Box", v(0.0, 0.0, 0.0), v(0.5, 0.25, 2.0));
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Box");
    let out = export(&model, 0.01);
    assert_eq!(out.objects.len(), 2);
    for (o, body) in out.objects.iter().zip([body, body2]) {
        let report = check_body(&model, body).unwrap();
        assert!(report.is_solid());
        assert_rel(volume(&o.triangles), report.volume.unwrap() * 1e9, 1e-9);
        assert_closed(&o.triangles);
    }
    // 4 x 4 x 1 m plate minus 2 x 2 x 1 m hole.
    assert_rel(volume(&out.objects[0].triangles), 12.0e9, 1e-9);
}

#[test]
fn a_hole_in_a_face_stays_a_hole() {
    let mut model = Model::new();
    let (def, _) = plate_with_hole(&mut model, "Plate");
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Plate");
    let out = export(&model, 0.01);
    let t = &out.objects[0].triangles;
    // No triangle's middle lies over the hole (x, y both in 1..3 m) on the top or bottom.
    for tri in t {
        let mid = (tri[0] + tri[1] + tri[2]) / 3.0;
        let flat = tri.iter().all(|p| p.z == tri[0].z);
        if flat {
            assert!(
                !(mid.x > 1000.0 && mid.x < 3000.0 && mid.y > 1000.0 && mid.y < 3000.0),
                "triangle covers the hole: {tri:?}"
            );
        }
    }
}

// --- transforms -------------------------------------------------------------------------

#[test]
fn instance_transforms_are_applied() {
    let mut model = Model::new();
    let m = DMat4::from_translation(v(2.0, 0.0, 0.5))
        * DMat4::from_quat(DQuat::from_rotation_z(FRAC_PI_2));
    unit_cube(&mut model, "Cube", m);
    let o = &export(&model, 0.01).objects[0];
    // Rotated a quarter turn about Z the cube covers x in 1..2, y in 0..1 before moving.
    let (lo, hi) = bounds(&o.triangles);
    assert_close(lo, v(1000.0, 0.0, 500.0), 1e-6);
    assert_close(hi, v(2000.0, 1000.0, 1500.0), 1e-6);
    assert_rel(volume(&o.triangles), 1e9, 1e-9);
    assert_closed(&o.triangles);
}

#[test]
fn scaled_instances_are_scaled() {
    let mut model = Model::new();
    unit_cube(&mut model, "Brick", DMat4::from_scale(v(2.0, 3.0, 4.0)));
    let o = &export(&model, 0.01).objects[0];
    let (lo, hi) = bounds(&o.triangles);
    assert_close(lo, DVec3::ZERO, 1e-9);
    assert_close(hi, v(2000.0, 3000.0, 4000.0), 1e-9);
    assert_rel(volume(&o.triangles), 24e9, 1e-9);
}

#[test]
fn mirrored_instances_still_face_outward() {
    let mut model = Model::new();
    unit_cube(&mut model, "Mirror", DMat4::from_scale(v(-1.0, 1.0, 1.0)));
    let o = &export(&model, 0.01).objects[0];
    let (lo, hi) = bounds(&o.triangles);
    assert_close(lo, v(-1000.0, 0.0, 0.0), 1e-9);
    assert_close(hi, v(0.0, 1000.0, 1000.0), 1e-9);
    assert_rel(volume(&o.triangles), 1e9, 1e-9);
    assert_outward(&o.triangles, v(-500.0, 500.0, 500.0));
    assert_closed(&o.triangles);
}

#[test]
fn two_instances_of_one_component_are_two_objects() {
    let mut model = Model::new();
    let (def, _, _) = box_group(&mut model, "Peg", v(0.0, 0.0, 0.0), v(0.01, 0.01, 0.02));
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Left");
    place(
        &mut model,
        BodyId::Root,
        def,
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
        "Right",
    );
    let out = export(&model, 0.01);
    let names: Vec<_> = out.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["Left", "Right"]);
    assert_close(bounds(&out.objects[0].triangles).0, DVec3::ZERO, 1e-9);
    assert_close(
        bounds(&out.objects[1].triangles).0,
        v(1000.0, 0.0, 0.0),
        1e-9,
    );
}

#[test]
fn an_unnamed_instance_takes_its_definition_name_and_then_object() {
    let mut model = Model::new();
    let (def, _, _) = box_group(&mut model, "Part", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "");
    let (def, _, _) = box_group(&mut model, "", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "");
    let names: Vec<_> = export(&model, 0.01)
        .objects
        .into_iter()
        .map(|o| o.name)
        .collect();
    assert_eq!(names, ["Part", "Object"]);
}

// --- nesting ----------------------------------------------------------------------------

#[test]
fn nested_instances_are_flattened_with_composed_transforms() {
    let mut model = Model::new();
    let (inner, _, _) = box_group(&mut model, "Inner", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    let (outer, outer_body) = definition(&mut model, "Outer", true);
    place(
        &mut model,
        outer_body,
        inner,
        DMat4::from_translation(v(1.0, 0.0, 0.0)) * DMat4::from_scale(DVec3::splat(2.0)),
        "Inner",
    );
    place(
        &mut model,
        BodyId::Root,
        outer,
        DMat4::from_translation(v(0.0, 5.0, 0.0)) * DMat4::from_scale(DVec3::splat(10.0)),
        "Outer",
    );
    let out = export(&model, 0.01);
    assert!(out.skipped.is_empty());
    assert_eq!(out.objects.len(), 1, "the empty outer group isn't listed");
    let o = &out.objects[0];
    assert_eq!(o.name, "Outer/Inner");
    // Inner: x 1..3, y 0..2, z 0..2 inside Outer; Outer scales by 10 and moves 5 m in y.
    let (lo, hi) = bounds(&o.triangles);
    assert_close(lo, v(10_000.0, 5_000.0, 0.0), 1e-6);
    assert_close(hi, v(30_000.0, 25_000.0, 20_000.0), 1e-6);
    assert_rel(volume(&o.triangles), 8.0 * 1000.0 * 1e9, 1e-9);
}

#[test]
fn a_group_with_faces_and_nested_parts_is_two_kinds_of_object() {
    let mut model = Model::new();
    let (inner, _, _) = box_group(&mut model, "Lid", v(0.0, 0.0, 0.0), v(0.5, 0.5, 0.5));
    let (outer, outer_body, _) = box_group(&mut model, "Base", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    place(
        &mut model,
        outer_body,
        inner,
        DMat4::from_translation(v(2.0, 0.0, 0.0)),
        "Lid",
    );
    place(&mut model, BodyId::Root, outer, DMat4::IDENTITY, "Base");
    let out = export(&model, 0.01);
    assert!(out.skipped.is_empty());
    let names: Vec<_> = out.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(
        names,
        ["Base", "Base/Lid"],
        "parent first, then what is nested"
    );
    assert_rel(volume(&out.objects[0].triangles), 1e9, 1e-9);
    assert_rel(volume(&out.objects[1].triangles), 0.125e9, 1e-9);
    assert_close(
        bounds(&out.objects[1].triangles).0,
        v(2000.0, 0.0, 0.0),
        1e-9,
    );
}

// --- orientation ------------------------------------------------------------------------

#[test]
fn a_reversed_face_is_turned_to_face_outward() {
    let mut model = Model::new();
    let (def, body, _) =
        box_group_turned(&mut model, "Cube", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0), &[1]);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Cube");
    let report = check_body(&model, body).unwrap();
    assert!(report.is_solid());
    assert_eq!(report.reversed_faces.len(), 1);
    let o = &export(&model, 0.01).objects[0];
    assert_outward(&o.triangles, v(500.0, 500.0, 500.0));
    assert_rel(volume(&o.triangles), 1e9, 1e-9);
    assert_closed(&o.triangles);
}

#[test]
fn a_box_wound_inside_out_is_turned_outward() {
    let mut model = Model::new();
    let (def, _, _) = box_group_turned(
        &mut model,
        "Cube",
        v(0.0, 0.0, 0.0),
        v(1.0, 1.0, 1.0),
        &[0, 1, 2, 3, 4, 5],
    );
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Cube");
    let o = &export(&model, 0.01).objects[0];
    assert_outward(&o.triangles, v(500.0, 500.0, 500.0));
    assert_rel(volume(&o.triangles), 1e9, 1e-9);
}

#[test]
fn a_reversed_face_in_a_mirrored_instance() {
    let mut model = Model::new();
    let (def, _, _) =
        box_group_turned(&mut model, "Cube", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0), &[2]);
    place(
        &mut model,
        BodyId::Root,
        def,
        DMat4::from_scale(v(1.0, -1.0, 1.0)),
        "Cube",
    );
    let o = &export(&model, 0.01).objects[0];
    assert_outward(&o.triangles, v(500.0, -500.0, 500.0));
    assert_closed(&o.triangles);
}

// --- what is left out ---------------------------------------------------------------------

#[test]
fn non_solids_are_skipped_with_the_reason() {
    let mut model = Model::new();
    unit_cube(&mut model, "Good", DMat4::IDENTITY);
    let open = open_box_group(&mut model, "Open");
    place(&mut model, BodyId::Root, open, DMat4::IDENTITY, "Open");
    let (empty, _) = definition(&mut model, "Nothing", true);
    place(&mut model, BodyId::Root, empty, DMat4::IDENTITY, "Nothing");
    let out = export(&model, 0.01);
    assert_eq!(out.objects.len(), 1);
    assert_eq!(out.objects[0].name, "Good");
    assert_eq!(out.skipped.len(), 2);
    assert_eq!(out.skipped[0].name, "Open");
    match &out.skipped[0].reason {
        SkipReason::NotSolid(report) => assert_eq!(report.open_edges.len(), 4),
        other => panic!("unexpected {other:?}"),
    }
    assert!(out.skipped[0].reason.to_string().contains("4 open edges"));
    assert_eq!(out.skipped[1].name, "Nothing");
    assert_eq!(out.skipped[1].reason, SkipReason::NoGeometry);
    assert_eq!(out.skipped[1].reason.to_string(), "has no faces");
}

#[test]
fn a_skipped_parent_still_exports_its_solid_children() {
    let mut model = Model::new();
    let (inner, _, _) = box_group(&mut model, "Inner", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    let open = open_box_group(&mut model, "Open");
    let (outer, outer_body) = definition(&mut model, "Outer", true);
    // Outer has no faces of its own: one open part and one solid part inside it.
    place(&mut model, outer_body, open, DMat4::IDENTITY, "Open");
    place(&mut model, outer_body, inner, DMat4::IDENTITY, "Inner");
    place(&mut model, BodyId::Root, outer, DMat4::IDENTITY, "Outer");
    let out = export(&model, 0.01);
    assert_eq!(out.objects.len(), 1);
    assert_eq!(out.objects[0].name, "Outer/Inner");
    assert_eq!(out.skipped.len(), 1);
    assert_eq!(out.skipped[0].name, "Outer/Open");
}

#[test]
fn loose_geometry_in_the_root_is_not_an_object() {
    let mut model = Model::new();
    let faces: Vec<FaceSpec> = vec![(&[0, 1, 2, 3], &[])];
    build(
        &mut model,
        BodyId::Root,
        &[
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
        &faces,
        &[],
    );
    let out = export(&model, 0.01);
    assert_eq!(out, PrintMesh::default());
}

#[test]
fn an_empty_model_gives_an_empty_result() {
    assert_eq!(export(&Model::new(), 0.01), PrintMesh::default());
}

// --- selection --------------------------------------------------------------------------

#[test]
fn a_selection_exports_only_those_instances() {
    let mut model = Model::new();
    let a = unit_cube(&mut model, "A", DMat4::IDENTITY);
    let b = unit_cube(&mut model, "B", DMat4::from_translation(v(2.0, 0.0, 0.0)));
    let c = unit_cube(&mut model, "C", DMat4::from_translation(v(4.0, 0.0, 0.0)));
    let open = open_box_group(&mut model, "Open");
    let d = place(&mut model, BodyId::Root, open, DMat4::IDENTITY, "Open");
    let options = PrintOptions::default();

    let all = print_mesh(&model, PrintScope::Model, &options);
    let names: Vec<_> = all.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["A", "B", "C"]);
    assert_eq!(all.skipped.len(), 1);

    // Selection order doesn't matter; the result follows instance ids.
    let some = print_mesh(&model, PrintScope::Instances(&[c, a]), &options);
    let names: Vec<_> = some.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["A", "C"]);
    assert!(some.skipped.is_empty(), "unselected parts aren't reported");

    let with_open = print_mesh(&model, PrintScope::Instances(&[b, d]), &options);
    assert_eq!(with_open.objects.len(), 1);
    assert_eq!(with_open.objects[0].name, "B");
    assert_eq!(with_open.skipped.len(), 1);
    assert_eq!(with_open.skipped[0].name, "Open");

    let none = print_mesh(&model, PrintScope::Instances(&[]), &options);
    assert_eq!(none, PrintMesh::default());
    let unknown = print_mesh(&model, PrintScope::Instances(&[InstanceId(9999)]), &options);
    assert_eq!(unknown, PrintMesh::default());
}

#[test]
fn selecting_a_container_takes_everything_inside_it() {
    let mut model = Model::new();
    let (inner, _, _) = box_group(&mut model, "Inner", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    let (outer, outer_body) = definition(&mut model, "Outer", true);
    place(&mut model, outer_body, inner, DMat4::IDENTITY, "One");
    place(
        &mut model,
        outer_body,
        inner,
        DMat4::from_translation(v(2.0, 0.0, 0.0)),
        "Two",
    );
    let id = place(&mut model, BodyId::Root, outer, DMat4::IDENTITY, "Outer");
    let out = print_mesh(
        &model,
        PrintScope::Instances(&[id]),
        &PrintOptions::default(),
    );
    let names: Vec<_> = out.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["Outer/One", "Outer/Two"]);
}

// --- colors -------------------------------------------------------------------------------

#[test]
fn unpainted_objects_have_no_color() {
    let mut model = Model::new();
    unit_cube(&mut model, "Cube", DMat4::IDENTITY);
    assert_eq!(export(&model, 0.01).objects[0].color, None);
}

#[test]
fn the_color_is_the_front_material_covering_the_most_area() {
    let mut model = Model::new();
    let red = add_material(&mut model, "Red", [1.0, 0.0, 0.0, 1.0]);
    let blue = add_material(&mut model, "Blue", [0.0, 0.0, 1.0, 0.5]);
    // 1 x 1 x 3 m: the four long sides are 3 m² each, the ends 1 m² each.
    let (def, body, faces) = box_group(&mut model, "Post", v(0.0, 0.0, 0.0), v(1.0, 1.0, 3.0));
    paint(&mut model, body, faces[0], Some(red), None); // bottom, 1 m²
    paint(&mut model, body, faces[1], Some(red), None); // top, 1 m²
    paint(&mut model, body, faces[2], Some(blue), None); // front, 3 m²
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Post");
    assert_eq!(
        export(&model, 0.01).objects[0].color,
        Some([0.0, 0.0, 1.0, 0.5])
    );
}

#[test]
fn area_is_measured_in_the_world_not_in_the_definition() {
    let mut model = Model::new();
    let red = add_material(&mut model, "Red", [1.0, 0.0, 0.0, 1.0]);
    let blue = add_material(&mut model, "Blue", [0.0, 0.0, 1.0, 1.0]);
    let (def, body, faces) = box_group(&mut model, "Cube", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    paint(&mut model, body, faces[0], Some(red), None); // bottom, 1 m² in the definition
    paint(&mut model, body, faces[2], Some(blue), None); // front, 1 m²
    // Stretched 10 times in x: the bottom becomes 10 m², the front 10 m² too; stretch in z
    // as well and the front (x by z) becomes 100 m² against the bottom's 10 m².
    place(
        &mut model,
        BodyId::Root,
        def,
        DMat4::from_scale(v(10.0, 1.0, 10.0)),
        "Cube",
    );
    assert_eq!(
        export(&model, 0.01).objects[0].color,
        Some([0.0, 0.0, 1.0, 1.0])
    );
}

#[test]
fn equal_areas_take_the_lowest_material_id() {
    let mut model = Model::new();
    let red = add_material(&mut model, "Red", [1.0, 0.0, 0.0, 1.0]);
    let blue = add_material(&mut model, "Blue", [0.0, 0.0, 1.0, 1.0]);
    let (def, body, faces) = box_group(&mut model, "Cube", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    paint(&mut model, body, faces[0], Some(blue), None);
    paint(&mut model, body, faces[1], Some(red), None);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Cube");
    assert_eq!(
        export(&model, 0.01).objects[0].color,
        Some([1.0, 0.0, 0.0, 1.0])
    );
}

#[test]
fn the_instance_material_paints_faces_without_their_own() {
    let mut model = Model::new();
    let red = add_material(&mut model, "Red", [1.0, 0.0, 0.0, 1.0]);
    let green = add_material(&mut model, "Green", [0.0, 1.0, 0.0, 1.0]);
    let (def, body, faces) = box_group(&mut model, "Cube", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    let id = place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Cube");
    set_instance_material(&mut model, BodyId::Root, id, Some(green));
    // One red face against five faces that take the instance's green.
    paint(&mut model, body, faces[0], Some(red), None);
    assert_eq!(
        export(&model, 0.01).objects[0].color,
        Some([0.0, 1.0, 0.0, 1.0])
    );
}

#[test]
fn a_nested_part_inherits_the_materials_of_the_instances_around_it() {
    let mut model = Model::new();
    let red = add_material(&mut model, "Red", [1.0, 0.0, 0.0, 1.0]);
    let green = add_material(&mut model, "Green", [0.0, 1.0, 0.0, 1.0]);
    let (inner, _, _) = box_group(&mut model, "Inner", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    let (outer, outer_body) = definition(&mut model, "Outer", true);
    place(&mut model, outer_body, inner, DMat4::IDENTITY, "Plain");
    let b = place(
        &mut model,
        outer_body,
        inner,
        DMat4::from_translation(v(2.0, 0.0, 0.0)),
        "Own",
    );
    set_instance_material(&mut model, outer_body, b, Some(green));
    let top = place(&mut model, BodyId::Root, outer, DMat4::IDENTITY, "Outer");
    set_instance_material(&mut model, BodyId::Root, top, Some(red));
    let out = export(&model, 0.01);
    assert_eq!(out.objects[0].name, "Outer/Plain");
    assert_eq!(out.objects[0].color, Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(out.objects[1].name, "Outer/Own");
    assert_eq!(out.objects[1].color, Some([0.0, 1.0, 0.0, 1.0]));
}

#[test]
fn a_turned_face_uses_its_back_material() {
    let mut model = Model::new();
    let red = add_material(&mut model, "Red", [1.0, 0.0, 0.0, 1.0]);
    let blue = add_material(&mut model, "Blue", [0.0, 0.0, 1.0, 1.0]);
    // Every face turned: the stored back is the side that ends up outside.
    let (def, body, faces) = box_group_turned(
        &mut model,
        "Cube",
        v(0.0, 0.0, 0.0),
        v(1.0, 1.0, 1.0),
        &[0, 1, 2, 3, 4, 5],
    );
    for f in faces {
        paint(&mut model, body, f, Some(red), Some(blue));
    }
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Cube");
    assert_eq!(
        export(&model, 0.01).objects[0].color,
        Some([0.0, 0.0, 1.0, 1.0])
    );
}

// --- curves -------------------------------------------------------------------------------

const R: f64 = 0.01; // 10 mm
const H: f64 = 0.02; // 20 mm

fn cylinder_model(n: usize, curves: bool, transform: DMat4) -> Model {
    let mut model = Model::new();
    let def = cylinder_group(&mut model, "Can", R, H, n, curves);
    place(&mut model, BodyId::Root, def, transform, "Can");
    model
}

fn triangles(model: &Model, tol_mm: f64) -> Vec<Triangle> {
    let out = export(model, tol_mm);
    assert!(out.skipped.is_empty(), "{:?}", out.skipped);
    out.objects.into_iter().next().unwrap().triangles
}

#[test]
fn without_curves_a_prism_is_exported_as_it_is() {
    let model = cylinder_model(24, false, DMat4::IDENTITY);
    let fine = triangles(&model, 0.001);
    let coarse = triangles(&model, 5.0);
    assert_eq!(fine, coarse);
    assert_eq!(fine.len(), 4 * 24 - 4);
    assert_closed(&fine);
}

#[test]
fn a_tolerance_the_polygon_already_meets_changes_nothing() {
    // 24 steps on a 10 mm radius: sagitta 10 * (1 - cos(7.5°)) = 0.0856 mm.
    let model = cylinder_model(24, true, DMat4::IDENTITY);
    let t = triangles(&model, 0.1);
    assert_eq!(t.len(), 4 * 24 - 4);
    let t2 = triangles(&model, 50.0);
    assert_eq!(t, t2);
}

#[test]
fn finer_tolerances_split_the_circle_further() {
    let model = cylinder_model(24, true, DMat4::IDENTITY);
    let mut last = 0;
    for tol in [0.1, 0.05, 0.02, 0.01, 0.005, 0.002, 0.001] {
        let t = triangles(&model, tol);
        assert!(t.len() >= last, "tolerance {tol}: {} < {last}", t.len());
        last = t.len();
        assert_closed(&t);
        for z in [0.0, 20.0] {
            let ring = ring_at(&t, z);
            let sagitta = max_sagitta(&ring, 10.0);
            assert!(sagitta <= tol + 1e-9, "tol {tol}: sagitta {sagitta}");
            for (x, y) in ring {
                assert!(((x * x + y * y).sqrt() - 10.0).abs() < 1e-9);
            }
        }
    }
    assert!(last > 4 * 24 - 4, "0.001 mm must split the circle");
}

#[test]
fn a_split_cylinder_has_the_right_shape() {
    let model = cylinder_model(24, true, DMat4::IDENTITY);
    let t = triangles(&model, 0.002);
    let ring = ring_at(&t, 0.0);
    let n = ring.len();
    assert!(
        n > 24 && n.is_multiple_of(24),
        "rings keep the original vertices: {n}"
    );
    assert_eq!(ring_at(&t, 20.0).len(), n);
    // Caps are n-gons, each side quad becomes a strip: 2 (n - 2) + 2 n triangles.
    assert_eq!(t.len(), 4 * n - 4);
    let v = volume(&t);
    let exact = PI * 100.0 * 20.0;
    assert!(
        v <= exact && v >= exact - 2.0 * PI * 10.0 * 0.002 * 20.0,
        "{v}"
    );
    let (lo, hi) = bounds(&t);
    assert!(lo.z.abs() < 1e-9 && (hi.z - 20.0).abs() < 1e-9);
    // Points on the original vertices are still there bit for bit.
    let first = v_at(&model);
    assert!(ring.iter().any(|&(x, y)| x == first.0 && y == first.1));
}

fn v_at(model: &Model) -> (f64, f64) {
    let ctx = model
        .body(BodyId::Definition(model.definitions().next().unwrap().0))
        .unwrap()
        .geometry();
    let p = ctx.vertices().next().unwrap().1.position;
    (p.x * 1000.0, p.y * 1000.0)
}

#[test]
fn a_bad_tolerance_is_read_as_the_smallest() {
    let model = cylinder_model(12, true, DMat4::IDENTITY);
    let smallest = triangles(&model, MIN_CHORD_TOL * 1000.0);
    for bad in [0.0, -1.0, 1e-9, f64::NAN, f64::NEG_INFINITY] {
        assert_eq!(triangles(&model, bad), smallest, "tolerance {bad}");
    }
    assert!(smallest.len() > 4 * 12 - 4);
    // An infinite tolerance is as coarse as it gets.
    assert_eq!(triangles(&model, f64::INFINITY).len(), 4 * 12 - 4);
}

#[test]
fn the_tolerance_is_measured_in_the_world() {
    // Ten times larger: radius 100 mm. The same 0.01 mm must hold in the printed size.
    let model = cylinder_model(24, true, DMat4::from_scale(DVec3::splat(10.0)));
    let t = triangles(&model, 0.01);
    for z in [0.0, 200.0] {
        let ring = ring_at(&t, z);
        assert!(max_sagitta(&ring, 100.0) <= 0.01 + 1e-9);
    }
    assert_closed(&t);
}

#[test]
fn stretched_curves_keep_the_tolerance_along_the_longest_axis() {
    let model = cylinder_model(24, true, DMat4::from_scale(v(3.0, 1.0, 1.0)));
    let t = triangles(&model, 0.01);
    assert_closed(&t);
    // Each printed chord stays within 0.01 mm of the stretched circle (an ellipse); the
    // chord midpoint is at most 3 x the local sagitta from it, so at the very least the
    // volume matches the ellipse cylinder closely.
    let exact = PI * 30.0 * 10.0 * 20.0;
    assert!((volume(&t) - exact).abs() < exact * 1e-3);
}

#[test]
fn mirrored_and_rotated_cylinders_are_closed_and_outward() {
    let m = DMat4::from_quat(DQuat::from_rotation_x(0.7)) * DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let model = cylinder_model(20, true, m);
    let t = triangles(&model, 0.01);
    assert_closed(&t);
    assert!(volume(&t) > 0.0);
}

#[test]
fn open_arcs_are_split_too() {
    let mut model = Model::new();
    let def = half_cylinder_group(&mut model, "D", R, H, 8);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "D");
    let coarse = triangles(&model, 50.0);
    assert_eq!(coarse.len(), 2 * (9 - 2) + 2 * 8 + 2);
    let fine = triangles(&model, 0.002);
    assert_closed(&fine);
    assert!(fine.len() > coarse.len());
    let exact = PI * 100.0 / 2.0 * 20.0;
    let v = volume(&fine);
    assert!(
        v <= exact && v >= exact - 10.0 * 2.0 * 0.002 * 20.0 * PI,
        "{v}"
    );
    // The straight wall is one flat quad: its two triangles stay two.
    let wall = fine
        .iter()
        .filter(|t| t.iter().all(|p| p.y.abs() < 1e-9))
        .count();
    assert_eq!(wall, 2);
}

#[test]
fn a_cylinder_is_a_solid_with_a_volume_close_to_the_report() {
    let model = cylinder_model(24, true, DMat4::IDENTITY);
    let report = check_body(
        &model,
        BodyId::Definition(model.definitions().next().unwrap().0),
    )
    .unwrap();
    let prism = report.volume.unwrap() * 1e9;
    // The unrefined polygon is inside the refined one.
    let fine = volume(&triangles(&model, 0.002));
    let coarse = volume(&triangles(&model, 50.0));
    assert_rel(coarse, prism, 1e-9);
    assert!(fine > coarse);
}

#[test]
fn a_cone_wall_is_a_fan_from_the_apex() {
    let mut model = Model::new();
    let def = cone_group(&mut model, "Cone", R, H, 24, true);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "Cone");
    let coarse = triangles(&model, 50.0);
    assert_eq!(coarse.len(), (24 - 2) + 24);
    let fine = triangles(&model, 0.002);
    assert_closed(&fine);
    let n = ring_at(&fine, 0.0).len();
    assert!(n > 24 && n.is_multiple_of(24));
    // Base is an n-gon, every wall piece reaches the apex: (n - 2) + n.
    assert_eq!(fine.len(), (n - 2) + n);
    let exact = PI * 100.0 * 20.0 / 3.0;
    let v = volume(&fine);
    assert!(
        v <= exact && v >= exact - 2.0 * PI * 10.0 * 0.002 * 20.0 / 3.0 - 1e-9,
        "{v}"
    );
}
