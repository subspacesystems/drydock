// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-clearance-ui: the fit check without a window (`docs/specs/m7-clearance-ui.md`). Two
//! real groups are built in code, checked, and the scene that would be uploaded is looked at.

use drydock::fit_check::{FitCheck, display_scene, format_mm, parse_clearance_mm, selected_pair};
use drydock_dae::{DEFAULT_CLEARANCE_M, FlatScene, kernel_scene};
use drydock_kernel::editpath::EditPath;
use drydock_kernel::model::{
    BodyId, DefinitionId, DefinitionInfo, Instance, InstanceId, Model, ModelEdit,
};
use drydock_kernel::select::Entity;
use drydock_kernel::topology::{Context, EdgeUse, Edit, Face, Loop, VertexId};
use glam::{DMat4, DVec3};
use std::borrow::Cow;

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn new_loop(ctx: &mut Context, log: &mut Edit, pts: &[DVec3]) -> Loop {
    let vs: Vec<VertexId> = pts.iter().map(|&q| ctx.insert_vertex(q, log)).collect();
    let uses = (0..vs.len())
        .map(|i| {
            let e = ctx
                .insert_edge(vs[i], vs[(i + 1) % vs.len()], log)
                .expect("edge inserts");
            EdgeUse::forward(e)
        })
        .collect();
    Loop { uses }
}

struct World {
    m: Model,
    log: ModelEdit,
}

impl World {
    fn new() -> World {
        World {
            m: Model::new(),
            log: ModelEdit::new(),
        }
    }

    /// A group definition holding a box from the origin to `size`, as six square faces.
    fn box_definition(&mut self, name: &str, size: DVec3) -> DefinitionId {
        let def = self.m.add_definition(
            DefinitionInfo {
                name: name.into(),
                group: true,
            },
            &mut self.log,
        );
        let c = |i: usize| {
            p(
                if i & 1 == 0 { 0.0 } else { size.x },
                if i & 2 == 0 { 0.0 } else { size.y },
                if i & 4 == 0 { 0.0 } else { size.z },
            )
        };
        let sides: [[usize; 4]; 6] = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [1, 3, 7, 5],
            [3, 2, 6, 7],
            [2, 0, 4, 6],
        ];
        self.m
            .edit_geometry(BodyId::Definition(def), &mut self.log, |ctx, log| {
                for s in sides {
                    let pts: Vec<DVec3> = s.iter().map(|&i| c(i)).collect();
                    let outer = new_loop(ctx, log, &pts);
                    ctx.insert_face(
                        Face {
                            outer,
                            inner: vec![],
                        },
                        log,
                    )
                    .expect("face inserts");
                }
            })
            .expect("body exists");
        def
    }

    fn place(&mut self, body: BodyId, def: DefinitionId, at: DVec3) -> InstanceId {
        self.m
            .add_instance(
                body,
                Instance {
                    definition: def,
                    transform: DMat4::from_translation(at),
                    name: String::new(),
                    material: None,
                },
                &mut self.log,
            )
            .expect("instance adds")
    }

    /// Two 10 mm cubes in the root, B `gap` meters from A along x. Returns (A, B).
    fn two_cubes(&mut self, gap: f64) -> (InstanceId, InstanceId) {
        let def = self.box_definition("cube", p(0.01, 0.01, 0.01));
        let a = self.place(BodyId::Root, def, p(0.0, 0.0, 0.0));
        let b = self.place(BodyId::Root, def, p(0.01 + gap, 0.0, 0.0));
        (a, b)
    }
}

fn pair(a: InstanceId, b: InstanceId) -> Vec<Entity> {
    vec![Entity::Instance(a), Entity::Instance(b)]
}

fn root_check(w: &World, a: InstanceId, b: InstanceId, clearance: f64) -> FitCheck {
    FitCheck::run(&w.m, &EditPath::root(), &pair(a, b), 7, clearance).expect("two parts")
}

// ---------------------------------------------------------------------------------------
// Pure helpers

#[test]
fn a_pair_is_exactly_two_different_instances() {
    let (a, b, c) = (InstanceId(1), InstanceId(2), InstanceId(3));
    assert_eq!(selected_pair(&pair(a, b)), Some((a, b)));
    assert_eq!(selected_pair(&pair(b, a)), Some((b, a)));
    assert_eq!(selected_pair(&[]), None);
    assert_eq!(selected_pair(&[Entity::Instance(a)]), None);
    assert_eq!(selected_pair(&pair(a, a)), None);
    assert_eq!(
        selected_pair(&[
            Entity::Instance(a),
            Entity::Instance(b),
            Entity::Instance(c)
        ]),
        None
    );
}

#[test]
fn a_pair_does_not_take_an_instance_with_something_else() {
    let (a, b) = (InstanceId(1), InstanceId(2));
    let face = Entity::Face(drydock_kernel::topology::FaceId(1));
    assert_eq!(selected_pair(&[Entity::Instance(a), face]), None);
    assert_eq!(selected_pair(&[face, Entity::Instance(b)]), None);
}

#[test]
fn the_field_accepts_millimeters_in_range() {
    let near = |text: &str, want: f64| {
        let got = parse_clearance_mm(text).unwrap_or_else(|| panic!("{text:?} rejected"));
        assert!((got - want).abs() < 1e-12, "{text:?} -> {got}");
    };
    near("0.2", 0.0002);
    near("  0.5  ", 0.0005);
    near("0.25 mm", 0.00025);
    near("1mm", 0.001);
    near("5", 0.005);
    near("0.01", 0.00001);
    near(".3", 0.0003);
}

#[test]
fn the_field_rejects_the_rest() {
    for text in [
        "", "  ", "abc", "0.2x", "mm", "0,5", "-1", "0", "0.009", "5.01", "100", "NaN", "inf",
        "1e-3",
    ] {
        assert_eq!(parse_clearance_mm(text), None, "{text:?}");
    }
}

#[test]
fn millimeters_are_written_short() {
    assert_eq!(format_mm(DEFAULT_CLEARANCE_M), "0.2");
    assert_eq!(format_mm(0.005), "5");
    assert_eq!(format_mm(0.00025), "0.25");
    assert_eq!(format_mm(0.00001), "0.01");
    assert_eq!(format_mm(0.0005), "0.5");
    assert_eq!(format_mm(0.001), "1");
    assert_eq!(format_mm(0.0012346), "1.235");
}

#[test]
fn what_the_field_writes_it_reads_back() {
    for mm in [0.01, 0.2, 0.25, 1.0, 3.5, 5.0] {
        let text = format_mm(mm / 1000.0);
        let back = parse_clearance_mm(&text).expect("reads");
        assert!((back - mm / 1000.0).abs() < 1e-12, "{mm} -> {text}");
    }
}

// ---------------------------------------------------------------------------------------
// Running the check

#[test]
fn cubes_a_tenth_of_a_millimeter_apart_are_flagged() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    assert_eq!((fit.a, fit.b), (a, b));
    assert_eq!(fit.path, EditPath::root());
    assert_eq!(fit.undo_state, 7);
    assert_eq!(fit.clearance_m, DEFAULT_CLEARANCE_M);
    // Five faces of each cube have an edge on the facing side (two triangles each); the far
    // face is more than the clearance away.
    assert_eq!((fit.in_a, fit.in_b), (10, 10));
    assert_eq!(fit.flagged.len(), 20);
    for (i, t) in fit.flagged.iter().enumerate() {
        if i < 10 {
            assert!(t.iter().any(|q| q.x > 0.0099), "A triangle {t:?}");
        } else {
            assert!(t.iter().any(|q| q.x < 0.0102), "B triangle {t:?}");
        }
    }
    assert_eq!(
        fit.message(),
        "10 triangles of A and 10 triangles of B are closer than 0.2 mm"
    );
}

#[test]
fn cubes_far_apart_report_nothing() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.001);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    assert!(fit.flagged.is_empty());
    assert_eq!((fit.in_a, fit.in_b), (0, 0));
    assert_eq!(fit.message(), "No triangles closer than 0.2 mm");
}

#[test]
fn the_clearance_decides() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.001);
    let fit = root_check(&w, a, b, 0.002);
    assert!(!fit.flagged.is_empty());
    assert!(
        fit.message().ends_with("are closer than 2 mm"),
        "{}",
        fit.message()
    );
    assert_eq!(fit.clearance_m, 0.002);
}

#[test]
fn touching_cubes_are_flagged_on_both_sides() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    assert!(fit.in_a >= 2 && fit.in_b >= 2);
}

#[test]
fn the_pair_keeps_its_selection_order() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    let fit = FitCheck::run(&w.m, &EditPath::root(), &pair(b, a), 0, DEFAULT_CLEARANCE_M)
        .expect("two parts");
    assert_eq!((fit.a, fit.b), (b, a));
}

#[test]
fn anything_but_two_instances_is_refused_with_a_message() {
    let mut w = World::new();
    let (a, _) = w.two_cubes(0.0001);
    let want = "Select two groups or components to check their fit";
    for selection in [vec![], vec![Entity::Instance(a)]] {
        let got = FitCheck::run(&w.m, &EditPath::root(), &selection, 0, DEFAULT_CLEARANCE_M);
        assert_eq!(got, Err(want.to_string()));
    }
}

#[test]
fn a_path_that_does_not_resolve_is_refused() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    let bad = EditPath {
        steps: vec![InstanceId(999)],
    };
    let got = FitCheck::run(&w.m, &bad, &pair(a, b), 0, DEFAULT_CLEARANCE_M);
    assert_eq!(got, Err("Can't check the fit here".to_string()));
}

#[test]
fn parts_inside_an_open_group_are_checked_in_world_space() {
    let mut w = World::new();
    let cube = w.box_definition("cube", p(0.01, 0.01, 0.01));
    let holder = w.m.add_definition(
        DefinitionInfo {
            name: "holder".into(),
            group: true,
        },
        &mut w.log,
    );
    let a = w.place(BodyId::Definition(holder), cube, p(0.0, 0.0, 0.0));
    let b = w.place(BodyId::Definition(holder), cube, p(0.0101, 0.0, 0.0));
    let outer = w.place(BodyId::Root, holder, p(1.0, 2.0, 3.0));
    let path = EditPath::root().open(&w.m, outer).expect("opens");
    let fit = FitCheck::run(&w.m, &path, &pair(a, b), 3, DEFAULT_CLEARANCE_M).expect("two parts");
    assert_eq!(fit.path, path);
    assert_eq!((fit.in_a, fit.in_b), (10, 10));
    for q in fit.flagged.iter().flatten() {
        assert!(
            q.x >= 1.0 - 1e-9 && q.y >= 2.0 - 1e-9 && q.z >= 3.0 - 1e-9,
            "{q:?}"
        );
        assert!(q.z <= 3.01 + 1e-9, "{q:?}");
    }
}

#[test]
fn recomputing_with_a_smaller_clearance_flags_less() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    let tighter = fit.recompute(&w.m, 0.00005);
    assert!(tighter.flagged.is_empty());
    assert_eq!(tighter.clearance_m, 0.00005);
    assert_eq!(
        (tighter.a, tighter.b, &tighter.path, tighter.undo_state),
        (a, b, &EditPath::root(), 7)
    );
    let looser = tighter.recompute(&w.m, DEFAULT_CLEARANCE_M);
    assert_eq!(looser, fit);
}

#[test]
fn recomputing_after_a_part_vanished_flags_nothing() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    let empty = Model::new();
    let again = fit.recompute(&empty, DEFAULT_CLEARANCE_M);
    assert!(again.flagged.is_empty());
}

// ---------------------------------------------------------------------------------------
// Staleness

#[test]
fn a_check_stays_current_until_something_changes() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    let root = EditPath::root();
    assert!(fit.is_current(&root, &pair(a, b), 7));
    assert!(
        fit.is_current(&root, &pair(b, a), 7),
        "order does not matter"
    );
    // An edit or an undo.
    assert!(!fit.is_current(&root, &pair(a, b), 8));
    // A different selection.
    assert!(!fit.is_current(&root, &[Entity::Instance(a)], 7));
    assert!(!fit.is_current(&root, &[], 7));
    assert!(!fit.is_current(&root, &pair(a, InstanceId(99)), 7));
    // Entering a group.
    let other = EditPath {
        steps: vec![InstanceId(99)],
    };
    assert!(!fit.is_current(&other, &pair(a, b), 7));
}

// ---------------------------------------------------------------------------------------
// What is uploaded

fn material_index(scene: &FlatScene, id: &str) -> Option<usize> {
    scene.materials.iter().position(|m| m.id == id)
}

fn triangles_in(scene: &FlatScene, material: usize) -> usize {
    scene
        .batches
        .iter()
        .filter(|b| b.material == Some(material))
        .map(|b| b.indices.len() / 3)
        .sum()
}

fn total_triangles(scene: &FlatScene) -> usize {
    scene.batches.iter().map(|b| b.indices.len() / 3).sum()
}

#[test]
fn without_tints_the_scene_is_passed_through() {
    let mut w = World::new();
    w.two_cubes(0.0001);
    let scene = kernel_scene(&w.m);
    assert!(matches!(
        display_scene(&scene, None, None),
        Cow::Borrowed(_)
    ));
}

#[test]
fn the_renderer_gets_a_fit_batch_with_the_flagged_faces() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    let scene = kernel_scene(&w.m);
    assert_eq!(total_triangles(&scene), 24);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    let shown = display_scene(&scene, None, Some(&fit));
    let k = material_index(&shown, "fit").expect("a fit material");
    assert_eq!(k, scene.materials.len());
    assert_eq!(shown.batches.last().expect("batches").material, Some(k));
    assert_eq!(triangles_in(&shown, k), 20);
    assert_eq!(total_triangles(&shown), 24);
    // The model's own scene is untouched.
    assert_eq!(total_triangles(&scene), 24);
    assert!(material_index(&scene, "fit").is_none());
}

#[test]
fn no_flagged_faces_leave_the_scene_as_it_was() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.001);
    let scene = kernel_scene(&w.m);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);
    let shown = display_scene(&scene, None, Some(&fit));
    assert_eq!(shown.materials.len(), scene.materials.len());
    assert!(material_index(&shown, "fit").is_none());
    assert_eq!(shown.batches.len(), scene.batches.len());
    assert_eq!(total_triangles(&shown), total_triangles(&scene));
}

#[test]
fn the_fit_tint_wins_over_the_overhang_tint() {
    let mut w = World::new();
    let (a, b) = w.two_cubes(0.0001);
    // A thin slab far below sets the bed, so the cubes' undersides count as overhangs.
    let slab = w.box_definition("slab", p(0.5, 0.5, 0.001));
    w.place(BodyId::Root, slab, p(-1.0, -1.0, -0.01));
    let scene = kernel_scene(&w.m);
    let fit = root_check(&w, a, b, DEFAULT_CLEARANCE_M);

    let overhangs_only = display_scene(&scene, Some(45.0), None);
    let over = material_index(&overhangs_only, "overhang").expect("overhang material");
    assert!(triangles_in(&overhangs_only, over) >= 4, "both undersides");

    let both = display_scene(&scene, Some(45.0), Some(&fit));
    let k = material_index(&both, "fit").expect("fit material");
    assert_eq!(shown_last(&both), Some(k), "fit batch is last");
    assert_eq!(triangles_in(&both, k), 20);
    // Every overhang triangle was flagged, so none is left in the overhang color.
    let left = material_index(&both, "overhang").map_or(0, |m| triangles_in(&both, m));
    assert_eq!(left, 0);
    assert_eq!(total_triangles(&both), total_triangles(&scene));
}

fn shown_last(scene: &FlatScene) -> Option<usize> {
    scene.batches.last().and_then(|b| b.material)
}

#[test]
fn overhang_alone_still_tints() {
    let mut w = World::new();
    w.two_cubes(0.001);
    let slab = w.box_definition("slab", p(0.5, 0.5, 0.001));
    w.place(BodyId::Root, slab, p(-1.0, -1.0, -0.01));
    let scene = kernel_scene(&w.m);
    let shown = display_scene(&scene, Some(45.0), None);
    let over = material_index(&shown, "overhang").expect("overhang material");
    assert!(triangles_in(&shown, over) >= 4);
    assert!(material_index(&shown, "fit").is_none());
}

// ---------------------------------------------------------------------------------------
// One upload path

#[test]
fn the_app_tints_in_one_place() {
    let app = include_str!("../src/app.rs");
    assert_eq!(
        app.matches("tint_overhangs(").count(),
        0,
        "use display_scene"
    );
    assert_eq!(app.matches("tint_fit(").count(), 0, "use display_scene");
    assert_eq!(app.matches("display_scene(").count(), 1);
    assert_eq!(
        app.matches("gpu.renderer.upload(").count(),
        1,
        "one method uploads the shown scene"
    );
}
