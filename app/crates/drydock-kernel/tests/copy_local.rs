// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Copying instances and exploding leave the rest of the body alone, and the checks after
//! an edit only look at the bodies it changed (`docs/specs/m4-fix-copy-speed.md`, part 2).

use std::collections::BTreeMap;
use std::time::Duration;

use drydock_kernel::component::make_component;
use drydock_kernel::copy::{copy_selection, extract, insert};
use drydock_kernel::explode::explode;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::topology::*;
use glam::{DMat4, DQuat, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

const CUBE_FACES: [&[usize]; 6] = [
    &[0, 3, 2, 1],
    &[4, 5, 6, 7],
    &[0, 1, 5, 4],
    &[2, 3, 7, 6],
    &[1, 2, 6, 5],
    &[3, 0, 4, 7],
];

/// A unit cube at `at` in `body`, built raw (no sticking). Returns its face ids.
fn cube_in(model: &mut Model, body: BodyId, at: DVec3) -> Vec<FaceId> {
    let mut points = Vec::new();
    for z in [0.0, 1.0] {
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            points.push(at + v(x, y, z));
        }
    }
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            let mut out = Vec::new();
            for face in CUBE_FACES {
                let mut uses = Vec::new();
                for i in 0..face.len() {
                    let (a, b) = (face[i], face[(i + 1) % face.len()]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], log).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                out.push(
                    ctx.insert_face(
                        Face {
                            outer: Loop { uses },
                            inner: Vec::new(),
                        },
                        log,
                    )
                    .unwrap(),
                );
            }
            out
        })
        .unwrap()
}

/// Break `body` on purpose, the way imported models often are: a second copy of one of
/// its faces, an edge crossing another one, a face whose loop crosses itself and a face
/// that isn't flat.
fn break_body(model: &mut Model, body: BodyId, face: FaceId, at: DVec3) {
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let copy = ctx.face(face).unwrap().clone();
            ctx.insert_face(copy, log).unwrap();
            let a = ctx.insert_vertex(at + v(-0.5, 0.5, 0.5), log);
            let b = ctx.insert_vertex(at + v(1.5, 0.5, 0.5), log);
            let c = ctx.insert_vertex(at + v(0.5, -0.5, 0.5), log);
            let d = ctx.insert_vertex(at + v(0.5, 1.5, 0.5), log);
            ctx.insert_edge(a, b, log).unwrap();
            ctx.insert_edge(c, d, log).unwrap();
            // A bow tie (its loop crosses itself) and a bent quad above the body.
            for (points, lift) in [
                (
                    [
                        v(0.0, 0.0, 0.0),
                        v(1.0, 1.0, 0.0),
                        v(1.0, 0.0, 0.0),
                        v(0.0, 1.0, 0.0),
                    ],
                    2.0,
                ),
                (
                    [
                        v(0.0, 0.0, 0.0),
                        v(1.0, 0.0, 0.3),
                        v(1.0, 1.0, 0.0),
                        v(0.0, 1.0, 0.3),
                    ],
                    3.0,
                ),
            ] {
                let vs: Vec<VertexId> = points
                    .iter()
                    .map(|p| ctx.insert_vertex(at + *p + v(0.0, 0.0, lift), log))
                    .collect();
                let mut uses = Vec::new();
                for i in 0..vs.len() {
                    let e = ctx.insert_edge(vs[i], vs[(i + 1) % vs.len()], log).unwrap();
                    uses.push(EdgeUse::forward(e));
                }
                ctx.insert_face(
                    Face {
                        outer: Loop { uses },
                        inner: Vec::new(),
                    },
                    log,
                )
                .unwrap();
            }
        })
        .unwrap();
}

fn definition(model: &mut Model, name: &str, group: bool) -> DefinitionId {
    let mut edit = ModelEdit::new();
    model.add_definition(
        DefinitionInfo {
            name: name.into(),
            group,
        },
        &mut edit,
    )
}

fn place(
    model: &mut Model,
    body: BodyId,
    definition: DefinitionId,
    transform: DMat4,
) -> InstanceId {
    let mut edit = ModelEdit::new();
    model
        .add_instance(
            body,
            Instance {
                definition,
                transform,
                name: "seat".into(),
                material: None,
            },
            &mut edit,
        )
        .unwrap()
}

fn faces_of(model: &Model, body: BodyId) -> Vec<(FaceId, Face, FaceMaterials)> {
    let target = model.body(body).unwrap();
    target
        .geometry()
        .faces()
        .map(|(id, face)| (id, face.clone(), target.face_materials(id)))
        .collect()
}

/// Root: a broken cube at the origin (duplicate face, crossing edges). Definition `chair`:
/// a cube, placed once in the root at (5, 0, 0).
fn house() -> (Model, DefinitionId, InstanceId) {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    break_body(&mut model, BodyId::Root, faces[1], DVec3::ZERO);
    let chair = definition(&mut model, "chair", false);
    cube_in(&mut model, BodyId::Definition(chair), DVec3::ZERO);
    let seat = place(
        &mut model,
        BodyId::Root,
        chair,
        DMat4::from_translation(v(5.0, 0.0, 0.0)),
    );
    (model, chair, seat)
}

#[test]
fn copying_an_instance_in_a_broken_body_succeeds_and_leaves_its_geometry_alone() {
    let (mut model, chair, seat) = house();
    assert!(!model.validate().is_empty());
    let before = model.clone();
    let geometry = model.body(BodyId::Root).unwrap().geometry().clone();
    let faces = faces_of(&model, BodyId::Root);
    let mut undo = UndoStack::new();
    let matrix = DMat4::from_translation(v(0.0, 3.0, 0.0));
    let got = copy_selection(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(seat)],
        matrix,
        &mut undo,
        "Copy",
    )
    .unwrap();
    assert_eq!(got.len(), 1);
    let Entity::Instance(copy) = got[0] else {
        panic!("expected an instance, got {got:?}");
    };
    assert_ne!(copy, seat);
    let root = model.body(BodyId::Root).unwrap();
    assert_eq!(root.geometry(), &geometry);
    assert_eq!(faces_of(&model, BodyId::Root), faces);
    let placed = root.instance(copy).unwrap();
    let original = root.instance(seat).unwrap();
    assert_eq!(placed.definition, chair);
    assert_eq!(placed.name, original.name);
    assert_eq!(placed.material, original.material);
    assert!(
        placed
            .transform
            .abs_diff_eq(matrix * original.transform, 1e-12)
    );
    assert_eq!(model.definitions().count(), before.definitions().count());
    assert_eq!(model.instances_of(chair).len(), 2);
    assert_eq!(undo.undo(&mut model).unwrap().as_deref(), Some("Copy"));
    assert!(model == before);
}

#[test]
fn copied_instances_share_their_definition_so_editing_it_changes_both() {
    let (mut model, chair, seat) = house();
    let mut undo = UndoStack::new();
    let turn = DMat4::from_rotation_translation(
        DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2),
        v(0.0, 4.0, 0.0),
    );
    copy_selection(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(seat)],
        turn,
        &mut undo,
        "Copy",
    )
    .unwrap();
    let users: Vec<InstanceId> = model
        .instances_of(chair)
        .into_iter()
        .map(|(body, id)| {
            assert_eq!(body, BodyId::Root);
            id
        })
        .collect();
    assert_eq!(users.len(), 2);
    assert_eq!(
        model
            .definitions()
            .filter(|(_, info)| info.name.starts_with("chair"))
            .count(),
        1
    );
}

#[test]
fn pasting_instances_into_another_broken_body_keeps_its_geometry() {
    let (mut model, chair, seat) = house();
    let room = definition(&mut model, "room", false);
    let room_faces = cube_in(&mut model, BodyId::Definition(room), v(0.0, 0.0, 0.0));
    break_body(
        &mut model,
        BodyId::Definition(room),
        room_faces[0],
        DVec3::ZERO,
    );
    place(
        &mut model,
        BodyId::Root,
        room,
        DMat4::from_translation(v(-5.0, 0.0, 0.0)),
    );
    let clipboard = extract(&model, BodyId::Root, &[Entity::Instance(seat)]).unwrap();
    let geometry = model
        .body(BodyId::Definition(room))
        .unwrap()
        .geometry()
        .clone();
    let mut undo = UndoStack::new();
    let got = insert(
        &mut model,
        BodyId::Definition(room),
        &clipboard,
        DMat4::from_translation(v(0.2, 0.2, 0.0)),
        &mut undo,
        "Paste",
    )
    .unwrap();
    assert_eq!(got.len(), 1);
    let Entity::Instance(pasted) = got[0] else {
        panic!("expected an instance, got {got:?}");
    };
    let body = model.body(BodyId::Definition(room)).unwrap();
    assert_eq!(body.geometry(), &geometry);
    assert_eq!(body.instance(pasted).unwrap().definition, chair);
    assert!(
        body.instance(pasted)
            .unwrap()
            .transform
            .abs_diff_eq(DMat4::from_translation(v(5.2, 0.2, 0.0)), 1e-12)
    );
}

#[test]
fn copying_a_group_with_broken_geometry_copies_it_as_it_is() {
    let mut model = Model::new();
    let shed = definition(&mut model, "shed", true);
    let faces = cube_in(&mut model, BodyId::Definition(shed), DVec3::ZERO);
    break_body(&mut model, BodyId::Definition(shed), faces[3], DVec3::ZERO);
    let placed = place(&mut model, BodyId::Root, shed, DMat4::IDENTITY);
    let source_violations = model.validate_bodies(&[BodyId::Definition(shed)]).len();
    assert!(source_violations > 0);
    let mut undo = UndoStack::new();
    let got = copy_selection(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(placed)],
        DMat4::from_translation(v(4.0, 0.0, 0.0)),
        &mut undo,
        "Copy",
    )
    .unwrap();
    let [Entity::Instance(copy)] = got[..] else {
        panic!("expected one instance, got {got:?}");
    };
    let new = model
        .body(BodyId::Root)
        .unwrap()
        .instance(copy)
        .unwrap()
        .definition;
    assert_ne!(new, shed);
    assert!(model.definition(new).unwrap().group);
    let source = model.body(BodyId::Definition(shed)).unwrap().geometry();
    let copied = model.body(BodyId::Definition(new)).unwrap().geometry();
    assert_eq!(copied.face_count(), source.face_count());
    assert_eq!(copied.edges().count(), source.edges().count());
    assert_eq!(
        model.validate_bodies(&[BodyId::Definition(new)]).len(),
        source_violations
    );
}

#[test]
fn copying_faces_and_an_instance_together_still_copies_both() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let chair = definition(&mut model, "chair", false);
    cube_in(&mut model, BodyId::Definition(chair), DVec3::ZERO);
    let seat = place(
        &mut model,
        BodyId::Root,
        chair,
        DMat4::from_translation(v(3.0, 0.0, 0.0)),
    );
    let mut selection: Vec<Entity> = faces.iter().map(|f| Entity::Face(*f)).collect();
    selection.push(Entity::Instance(seat));
    let mut undo = UndoStack::new();
    let got = copy_selection(
        &mut model,
        BodyId::Root,
        &selection,
        DMat4::from_translation(v(0.0, 10.0, 0.0)),
        &mut undo,
        "Copy",
    )
    .unwrap();
    assert_eq!(
        got.iter().filter(|e| matches!(e, Entity::Face(_))).count(),
        6
    );
    let instances: Vec<InstanceId> = got
        .iter()
        .filter_map(|e| match e {
            Entity::Instance(id) => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(instances.len(), 1);
    let root = model.body(BodyId::Root).unwrap();
    assert_eq!(root.instance(instances[0]).unwrap().definition, chair);
    assert!(
        root.instance(instances[0])
            .unwrap()
            .transform
            .abs_diff_eq(DMat4::from_translation(v(3.0, 10.0, 0.0)), 1e-12)
    );
    assert_eq!(root.geometry().face_count(), 12);
    assert!(model.validate().is_empty());
    let mut sorted = got.clone();
    sorted.sort();
    assert_eq!(got, sorted);
}

#[test]
fn exploding_beside_broken_geometry_keeps_every_untouched_face() {
    let (mut model, _chair, seat) = house();
    let faces = faces_of(&model, BodyId::Root);
    let violations = model.validate();
    let mut undo = UndoStack::new();
    let got = explode(&mut model, BodyId::Root, seat, &mut undo, "Explode").unwrap();
    assert_eq!(
        got.iter().filter(|e| matches!(e, Entity::Face(_))).count(),
        6
    );
    let now: BTreeMap<FaceId, (Face, FaceMaterials)> = faces_of(&model, BodyId::Root)
        .into_iter()
        .map(|(id, face, materials)| (id, (face, materials)))
        .collect();
    for (id, face, materials) in &faces {
        assert_eq!(
            now.get(id),
            Some(&(face.clone(), *materials)),
            "face {id:?}"
        );
    }
    assert_eq!(now.len(), faces.len() + 6);
    for violation in &violations {
        assert!(model.validate().contains(violation));
    }
}

#[test]
fn exploding_an_instance_that_only_holds_instances_changes_no_geometry() {
    let (mut model, chair, _seat) = house();
    let set = definition(&mut model, "dining set", false);
    place(
        &mut model,
        BodyId::Definition(set),
        chair,
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
    );
    place(
        &mut model,
        BodyId::Definition(set),
        chair,
        DMat4::from_translation(v(3.0, 0.0, 0.0)),
    );
    let table = place(
        &mut model,
        BodyId::Root,
        set,
        DMat4::from_translation(v(0.0, 8.0, 0.0)),
    );
    let geometry = model.body(BodyId::Root).unwrap().geometry().clone();
    let faces = faces_of(&model, BodyId::Root);
    let mut undo = UndoStack::new();
    let got = explode(&mut model, BodyId::Root, table, &mut undo, "Explode").unwrap();
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|e| matches!(e, Entity::Instance(_))));
    assert_eq!(model.body(BodyId::Root).unwrap().geometry(), &geometry);
    assert_eq!(faces_of(&model, BodyId::Root), faces);
}

#[test]
fn validate_bodies_reports_what_validate_reports_for_those_bodies() {
    let (mut model, chair, _seat) = house();
    let chair_faces: Vec<FaceId> = model
        .body(BodyId::Definition(chair))
        .unwrap()
        .geometry()
        .faces()
        .map(|(id, _)| id)
        .collect();
    break_body(
        &mut model,
        BodyId::Definition(chair),
        chair_faces[2],
        DVec3::ZERO,
    );
    let lonely = definition(&mut model, "lonely group", true);
    cube_in(&mut model, BodyId::Definition(lonely), DVec3::ZERO);
    let all = model.validate();
    let body_of = |violation: &ModelViolation| match violation {
        ModelViolation::Geometry(body, _) | ModelViolation::BadTransform(body, _) => *body,
        ModelViolation::GroupInstances(definition, _) => BodyId::Definition(*definition),
    };
    for bodies in [
        vec![BodyId::Root],
        vec![BodyId::Definition(chair)],
        vec![BodyId::Definition(lonely)],
        vec![BodyId::Root, BodyId::Definition(lonely)],
        vec![],
    ] {
        let want: Vec<ModelViolation> = all
            .iter()
            .filter(|violation| bodies.contains(&body_of(violation)))
            .cloned()
            .collect();
        assert_eq!(model.validate_bodies(&bodies), want, "{bodies:?}");
    }
    let every: Vec<BodyId> = std::iter::once(BodyId::Root)
        .chain(model.definitions().map(|(d, _)| BodyId::Definition(d)))
        .collect();
    assert_eq!(model.validate_bodies(&every), all);
    assert!(
        model
            .validate_bodies(&[BodyId::Definition(DefinitionId(999))])
            .is_empty()
    );
}

/// A definition nobody places, holding an `n` × `n` grid of squares: thousands of edges
/// that an edit elsewhere has no reason to look at.
fn add_large_unrelated_body(model: &mut Model, n: usize) {
    let big = definition(model, "warehouse", false);
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(BodyId::Definition(big), &mut edit, |ctx, log| {
            let mut vs = Vec::new();
            for y in 0..=n {
                for x in 0..=n {
                    vs.push(ctx.insert_vertex(v(x as f64, y as f64, -50.0), log));
                }
            }
            let at = |x: usize, y: usize| y * (n + 1) + x;
            for y in 0..=n {
                for x in 0..=n {
                    if x < n {
                        ctx.insert_edge(vs[at(x, y)], vs[at(x + 1, y)], log)
                            .unwrap();
                    }
                    if y < n {
                        ctx.insert_edge(vs[at(x, y)], vs[at(x, y + 1)], log)
                            .unwrap();
                    }
                }
            }
        })
        .unwrap();
}

/// Runs `f` on another thread and fails if it takes longer than `limit`.
fn within<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(f());
    });
    receive
        .recv_timeout(limit)
        .unwrap_or_else(|_| panic!("took longer than {limit:?}"))
}

#[test]
fn copy_explode_and_make_component_ignore_large_unrelated_bodies() {
    let (mut model, _chair, seat) = house();
    add_large_unrelated_body(&mut model, 90);
    let loose = cube_in(&mut model, BodyId::Root, v(0.0, -5.0, 0.0));
    within(Duration::from_secs(10), move || {
        let mut undo = UndoStack::new();
        copy_selection(
            &mut model,
            BodyId::Root,
            &[Entity::Instance(seat)],
            DMat4::from_translation(v(0.0, 2.0, 0.0)),
            &mut undo,
            "Copy",
        )
        .unwrap();
        let selection: Vec<Entity> = loose.iter().map(|f| Entity::Face(*f)).collect();
        let made = make_component(
            &mut model,
            BodyId::Root,
            &selection,
            DefinitionInfo {
                name: "box".into(),
                group: false,
            },
            &mut undo,
            "Make Component",
        )
        .unwrap();
        explode(&mut model, BodyId::Root, made, &mut undo, "Explode").unwrap();
        explode(&mut model, BodyId::Root, seat, &mut undo, "Explode").unwrap();
    });
}
