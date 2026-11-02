// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Cached flattening for pick and snap (`docs/specs/m2-infer-cache.md`).

use drydock_kernel::infer::*;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use glam::{DMat4, DVec2, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn top_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y) * 10.0)
}

fn top(x: f64, y: f64) -> Probe<'static> {
    Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: v(x, y, 100.0),
        dir: v(0.0, 0.0, -1.0),
        parallel: true,
        project: &top_project,
        radius: 5.0,
        anchor: None,
        axes: [DVec3::X, DVec3::Y, DVec3::Z],
        centers: &[],
        lock: None,
    }
}

/// A definition holding a 2 m square at height `z`.
fn square_def(m: &mut Model, z: f64) -> DefinitionId {
    let mut log = ModelEdit::new();
    let d = m.add_definition(
        DefinitionInfo {
            name: "part".into(),
            group: false,
        },
        &mut log,
    );
    let pts = [
        v(0.0, 0.0, z),
        v(2.0, 0.0, z),
        v(2.0, 2.0, z),
        v(0.0, 2.0, z),
    ];
    m.edit_geometry(BodyId::Definition(d), &mut log, |ctx, edit| {
        for i in 0..4 {
            ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
        }
    })
    .unwrap();
    d
}

fn place(m: &mut Model, d: DefinitionId, x: f64) -> InstanceId {
    let mut log = ModelEdit::new();
    m.add_instance(
        BodyId::Root,
        Instance {
            definition: d,
            transform: DMat4::from_translation(v(x, 0.0, 0.0)),
            name: String::new(),
            material: None,
        },
        &mut log,
    )
    .unwrap()
}

#[test]
fn revision_is_stable_for_reads() {
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    place(&mut m, d, 10.0);
    let r = m.revision();
    let _ = pick_model(&m, &top(11.0, 1.0));
    let _ = infer_model(&m, &top(11.0, 1.0));
    let _ = m.validate();
    assert_eq!(m.revision(), r);
    assert_eq!(m.clone().revision(), r);
}

#[test]
fn revision_changes_on_every_kind_of_edit() {
    let mut m = Model::new();
    let mut seen = vec![m.revision()];
    let mut check = |m: &Model, what: &str| {
        let r = m.revision();
        assert!(!seen.contains(&r), "{what} did not change the revision");
        seen.push(r);
    };

    let d = square_def(&mut m, 0.0);
    check(&m, "definition with geometry");
    let i = place(&mut m, d, 10.0);
    check(&m, "add_instance");

    let mut log = ModelEdit::new();
    let mut inst = m.root().instance(i).unwrap().clone();
    inst.transform = DMat4::from_translation(v(20.0, 0.0, 0.0));
    m.set_instance(BodyId::Root, i, inst, &mut log).unwrap();
    check(&m, "set_instance");

    let mut log = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
        ctx.add_edge(v(0.0, 5.0, 0.0), v(1.0, 5.0, 0.0), edit)
            .unwrap();
    })
    .unwrap();
    check(&m, "edit_geometry on the root");

    let mut log = ModelEdit::new();
    m.remove_instance(BodyId::Root, i, &mut log).unwrap();
    check(&m, "remove_instance");
}

#[test]
fn revision_changes_on_undo_and_redo() {
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    let mut stack = UndoStack::new();
    let mut log = ModelEdit::new();
    m.add_instance(
        BodyId::Root,
        Instance {
            definition: d,
            transform: DMat4::IDENTITY,
            name: String::new(),
            material: None,
        },
        &mut log,
    )
    .unwrap();
    stack.push("place", log);
    let after = m.revision();
    stack.undo(&mut m).unwrap();
    let undone = m.revision();
    assert_ne!(undone, after);
    stack.redo(&mut m).unwrap();
    assert_ne!(m.revision(), undone);
}

#[test]
fn queries_follow_edits() {
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), None);
    let i = place(&mut m, d, 10.0);
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), Some(Entity::Instance(i)));
    let mut log = ModelEdit::new();
    let mut inst = m.root().instance(i).unwrap().clone();
    inst.transform = DMat4::from_translation(v(30.0, 0.0, 0.0));
    m.set_instance(BodyId::Root, i, inst, &mut log).unwrap();
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), None);
    assert_eq!(pick_model(&m, &top(31.0, 1.0)), Some(Entity::Instance(i)));
    let hit = infer_model(&m, &top(30.1, 0.1)).unwrap();
    assert!(matches!(hit.snap, Snap::Endpoint(_)), "{:?}", hit.snap);
}

#[test]
fn queries_follow_undo() {
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    let mut stack = UndoStack::new();
    let mut log = ModelEdit::new();
    let i = m
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::from_translation(v(10.0, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    stack.push("place", log);
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), Some(Entity::Instance(i)));
    stack.undo(&mut m).unwrap();
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), None);
    stack.redo(&mut m).unwrap();
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), Some(Entity::Instance(i)));
}

/// Two models that share a history and then take the same number of different edits can have
/// equal revisions; a cache keyed on the revision alone would answer one with the other's data.
#[test]
fn clones_that_diverge_do_not_share_answers() {
    let mut a = Model::new();
    let d = square_def(&mut a, 0.0);
    place(&mut a, d, 10.0);
    let mut b = a.clone();
    assert!(pick_model(&a, &top(11.0, 1.0)).is_some());

    let ib = place(&mut b, d, 50.0);
    let ia = place(&mut a, d, 80.0);
    for _ in 0..3 {
        assert_eq!(pick_model(&b, &top(51.0, 1.0)), Some(Entity::Instance(ib)));
        assert_eq!(pick_model(&a, &top(51.0, 1.0)), None);
        assert_eq!(pick_model(&a, &top(81.0, 1.0)), Some(Entity::Instance(ia)));
        assert_eq!(pick_model(&b, &top(81.0, 1.0)), None);
    }
}

#[test]
fn models_on_different_threads_do_not_interfere() {
    let handles: Vec<_> = (0..4)
        .map(|k| {
            std::thread::spawn(move || {
                let mut m = Model::new();
                let d = square_def(&mut m, 0.0);
                let x = 10.0 * (k as f64 + 1.0);
                let i = place(&mut m, d, x);
                for _ in 0..50 {
                    assert_eq!(
                        pick_model(&m, &top(x + 1.0, 1.0)),
                        Some(Entity::Instance(i))
                    );
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
}
