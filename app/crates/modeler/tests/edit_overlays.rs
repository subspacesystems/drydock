// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Selection highlight in world coordinates, also inside an open instance
//! (`docs/specs/m4-fix-edit-overlays.md`).

use glam::{DMat4, DQuat, DVec3};
use modeler::overlay::*;
use modeler_kernel::model::*;
use modeler_kernel::select::{Entity, instance_segments};
use modeler_kernel::topology::{EdgeId, FaceId};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn near(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

fn square(c: [f64; 2], size: f64) -> [DVec3; 4] {
    [
        v(c[0], c[1], 0.0),
        v(c[0] + size, c[1], 0.0),
        v(c[0] + size, c[1] + size, 0.0),
        v(c[0], c[1] + size, 0.0),
    ]
}

fn add_loop(model: &mut Model, body: BodyId, pts: &[DVec3], log: &mut ModelEdit) {
    model
        .edit_geometry(body, log, |ctx, edit| {
            for i in 0..pts.len() {
                ctx.add_edge(pts[i], pts[(i + 1) % pts.len()], edit)
                    .unwrap();
            }
        })
        .unwrap();
}

struct World {
    model: Model,
    def: DefinitionId,
    /// An instance of `inner` placed inside `def`.
    nested: InstanceId,
}

/// `def` holds a 4 m square face (0,0)-(4,4) with a 2 m square hole (1,1)-(3,3) (the hole
/// is a face of its own), and an instance of `inner` (a 1 m square at the origin) moved to
/// (6, 0, 0). The root holds one instance of `def` (its transform is up to each test).
fn world(transform: DMat4) -> World {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "wall".into(),
            group: false,
        },
        &mut log,
    );
    let inner = model.add_definition(
        DefinitionInfo {
            name: "tile".into(),
            group: false,
        },
        &mut log,
    );
    add_loop(
        &mut model,
        BodyId::Definition(def),
        &square([0.0, 0.0], 4.0),
        &mut log,
    );
    add_loop(
        &mut model,
        BodyId::Definition(def),
        &square([1.0, 1.0], 2.0),
        &mut log,
    );
    add_loop(
        &mut model,
        BodyId::Definition(inner),
        &square([0.0, 0.0], 1.0),
        &mut log,
    );
    let nested = model
        .add_instance(
            BodyId::Definition(def),
            Instance {
                definition: inner,
                transform: DMat4::from_translation(v(6.0, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    World { model, def, nested }
}

fn body(w: &World) -> BodyId {
    BodyId::Definition(w.def)
}

fn edge_between(w: &World, a: DVec3, b: DVec3) -> EdgeId {
    let g = w.model.body(body(w)).unwrap().geometry();
    g.edges()
        .find(|(_, e)| {
            let p = g.vertex(e.ends[0]).unwrap().position;
            let q = g.vertex(e.ends[1]).unwrap().position;
            (near(p, a) && near(q, b)) || (near(p, b) && near(q, a))
        })
        .map(|(id, _)| id)
        .expect("edge")
}

/// The face with a hole (`holed`) or the face filling the hole (`!holed`).
fn face(w: &World, holed: bool) -> FaceId {
    let g = w.model.body(body(w)).unwrap().geometry();
    g.faces()
        .find(|(_, f)| f.inner.is_empty() != holed)
        .map(|(id, _)| id)
        .expect("face")
}

fn lines(prims: &[Primitive], style: Style) -> Vec<(DVec3, DVec3)> {
    prims
        .iter()
        .filter_map(|p| match p {
            Primitive::Line { a, b, style: s } if *s == style => Some((*a, *b)),
            _ => None,
        })
        .collect()
}

fn triangles(prims: &[Primitive]) -> Vec<[DVec3; 3]> {
    prims
        .iter()
        .filter_map(|p| match p {
            Primitive::Polygon { points, style } if *style == Style::SelectedFace => {
                assert_eq!(points.len(), 3, "a selected face is drawn as triangles");
                Some([points[0], points[1], points[2]])
            }
            _ => None,
        })
        .collect()
}

fn area(t: &[DVec3; 3]) -> f64 {
    (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0
}

fn same_segments(got: &[(DVec3, DVec3)], want: &[(DVec3, DVec3)]) -> bool {
    got.len() == want.len()
        && want.iter().all(|(a, b)| {
            got.iter()
                .any(|(p, q)| (near(*p, *a) && near(*q, *b)) || (near(*p, *b) && near(*q, *a)))
        })
}

fn moved() -> DMat4 {
    DMat4::from_translation(v(10.0, 20.0, 3.0))
}

fn turned() -> DMat4 {
    DMat4::from_scale_rotation_translation(
        DVec3::splat(2.0),
        DQuat::from_rotation_z(0.5) * DQuat::from_rotation_x(0.3),
        v(-5.0, 1.0, 2.0),
    )
}

#[test]
fn a_selected_edge_is_one_line_in_world_coordinates() {
    let w = world(moved());
    let e = edge_between(&w, v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    let prims = selection(&w.model, body(&w), moved(), &[Entity::Edge(e)]);
    assert_eq!(prims.len(), 1);
    let got = lines(&prims, Style::Selected);
    assert!(same_segments(
        &got,
        &[(v(10.0, 20.0, 3.0), v(14.0, 20.0, 3.0))]
    ));
}

#[test]
fn edges_follow_rotation_and_scale() {
    let w = world(turned());
    let (a, b) = (v(4.0, 0.0, 0.0), v(4.0, 4.0, 0.0));
    let e = edge_between(&w, a, b);
    let got = lines(
        &selection(&w.model, body(&w), turned(), &[Entity::Edge(e)]),
        Style::Selected,
    );
    assert!(same_segments(
        &got,
        &[(turned().transform_point3(a), turned().transform_point3(b))]
    ));
}

#[test]
fn identity_transform_draws_body_coordinates() {
    let w = world(moved());
    let e = edge_between(&w, v(1.0, 1.0, 0.0), v(3.0, 1.0, 0.0));
    let got = lines(
        &selection(&w.model, body(&w), DMat4::IDENTITY, &[Entity::Edge(e)]),
        Style::Selected,
    );
    assert!(same_segments(&got, &[(v(1.0, 1.0, 0.0), v(3.0, 1.0, 0.0))]));
}

#[test]
fn a_selected_face_is_a_tint_of_triangles_covering_it() {
    let w = world(moved());
    let f = face(&w, false);
    let prims = selection(&w.model, body(&w), moved(), &[Entity::Face(f)]);
    let tris = triangles(&prims);
    assert!(!tris.is_empty());
    assert_eq!(tris.len(), prims.len(), "only triangles for a face");
    let total: f64 = tris.iter().map(area).sum();
    assert!((total - 4.0).abs() < 1e-9, "area {total}");
    for t in &tris {
        for p in t {
            assert!((p.z - 3.0).abs() < 1e-9);
            assert!(p.x >= 11.0 - 1e-9 && p.x <= 13.0 + 1e-9, "{p}");
            assert!(p.y >= 21.0 - 1e-9 && p.y <= 23.0 + 1e-9, "{p}");
        }
    }
}

#[test]
fn a_face_with_a_hole_leaves_the_hole_untinted() {
    let w = world(moved());
    let f = face(&w, true);
    let tris = triangles(&selection(&w.model, body(&w), moved(), &[Entity::Face(f)]));
    let total: f64 = tris.iter().map(area).sum();
    assert!((total - 12.0).abs() < 1e-9, "area {total}");
    for t in &tris {
        let c = (t[0] + t[1] + t[2]) / 3.0;
        let inside_hole = c.x > 11.0 && c.x < 13.0 && c.y > 21.0 && c.y < 23.0;
        assert!(!inside_hole, "triangle centroid {c} is in the hole");
    }
}

#[test]
fn face_area_scales_with_the_transform() {
    let w = world(turned());
    let f = face(&w, true);
    let tris = triangles(&selection(&w.model, body(&w), turned(), &[Entity::Face(f)]));
    let total: f64 = tris.iter().map(area).sum();
    // Uniform scale 2: area times 4.
    assert!((total - 48.0).abs() < 1e-6, "area {total}");
}

#[test]
fn a_mirroring_transform_still_covers_the_face() {
    let mirror = DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let w = world(mirror);
    let f = face(&w, true);
    let tris = triangles(&selection(&w.model, body(&w), mirror, &[Entity::Face(f)]));
    let total: f64 = tris.iter().map(area).sum();
    assert!((total - 12.0).abs() < 1e-9, "area {total}");
    for t in &tris {
        for p in t {
            assert!(p.x <= 1e-9, "mirrored to negative x: {p}");
        }
    }
}

#[test]
fn a_selected_instance_draws_its_edges_mapped_to_the_world() {
    let w = world(turned());
    let prims = selection(&w.model, body(&w), turned(), &[Entity::Instance(w.nested)]);
    let got = lines(&prims, Style::Selected);
    let want: Vec<(DVec3, DVec3)> = instance_segments(&w.model, body(&w), w.nested)
        .into_iter()
        .map(|(a, b)| (turned().transform_point3(a), turned().transform_point3(b)))
        .collect();
    assert_eq!(want.len(), 4);
    assert!(same_segments(&got, &want));
    assert_eq!(got.len(), prims.len());
}

#[test]
fn several_entities_are_all_drawn() {
    let w = world(moved());
    let e = edge_between(&w, v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    let f = face(&w, false);
    let prims = selection(
        &w.model,
        body(&w),
        moved(),
        &[Entity::Edge(e), Entity::Face(f), Entity::Instance(w.nested)],
    );
    assert_eq!(lines(&prims, Style::Selected).len(), 1 + 4);
    assert!(!triangles(&prims).is_empty());
}

#[test]
fn missing_entities_and_bodies_draw_nothing() {
    let w = world(moved());
    let e = edge_between(&w, v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    // The root has no geometry and no such instance.
    assert!(
        selection(
            &w.model,
            BodyId::Root,
            DMat4::IDENTITY,
            &[Entity::Edge(e), Entity::Instance(w.nested)]
        )
        .is_empty()
    );
    assert!(
        selection(
            &w.model,
            BodyId::Definition(DefinitionId(w.def.0 + 99)),
            DMat4::IDENTITY,
            &[Entity::Edge(e)]
        )
        .is_empty()
    );
    assert!(selection(&w.model, body(&w), moved(), &[]).is_empty());
}

#[test]
fn transformed_maps_every_point_and_keeps_styles() {
    let t = turned();
    let prims = vec![
        Primitive::Line {
            a: v(1.0, 0.0, 0.0),
            b: v(0.0, 1.0, 0.0),
            style: Style::FaceHighlight,
        },
        Primitive::Polygon {
            points: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 0.0, 1.0)],
            style: Style::BoundsBox,
        },
        Primitive::Marker {
            at: v(2.0, 3.0, 4.0),
            style: Style::Handle,
        },
    ];
    let got = transformed(prims, t);
    let want = vec![
        Primitive::Line {
            a: t.transform_point3(v(1.0, 0.0, 0.0)),
            b: t.transform_point3(v(0.0, 1.0, 0.0)),
            style: Style::FaceHighlight,
        },
        Primitive::Polygon {
            points: vec![
                t.transform_point3(v(0.0, 0.0, 0.0)),
                t.transform_point3(v(1.0, 0.0, 0.0)),
                t.transform_point3(v(0.0, 0.0, 1.0)),
            ],
            style: Style::BoundsBox,
        },
        Primitive::Marker {
            at: t.transform_point3(v(2.0, 3.0, 4.0)),
            style: Style::Handle,
        },
    ];
    assert_eq!(got, want);
}

#[test]
fn transformed_with_identity_changes_nothing() {
    let prims = vec![Primitive::Marker {
        at: v(2.0, 3.0, 4.0),
        style: Style::GuidePoint,
    }];
    assert_eq!(transformed(prims.clone(), DMat4::IDENTITY), prims);
}
