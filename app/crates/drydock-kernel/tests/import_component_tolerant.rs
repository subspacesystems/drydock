// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-import-component-tolerant: a source model that already breaks some invariants still
//! imports as a component; only problems the import itself would add are refused.
//! See docs/specs/m10-import-component-tolerant.md.

use drydock_kernel::copy::import_component;
use drydock_kernel::model::*;
use drydock_kernel::topology::{EdgeId, Violation};
use glam::{DMat4, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn define(model: &mut Model, name: &str, group: bool) -> DefinitionId {
    let mut log = ModelEdit::new();
    model.add_definition(
        DefinitionInfo {
            name: name.to_string(),
            group,
        },
        &mut log,
    )
}

/// Put two edges that cross at their middles into `body`, bypassing the sticky drawing
/// rules (raw inserts), offset by `at`. Afterwards `body` has an `EdgesIntersect`.
fn add_crossing(model: &mut Model, body: BodyId, at: DVec3) {
    let mut log = ModelEdit::new();
    model
        .edit_geometry(body, &mut log, |ctx, edit| {
            let a = ctx.insert_vertex(at + v(0.0, 0.0, 0.0), edit);
            let b = ctx.insert_vertex(at + v(2.0, 2.0, 0.0), edit);
            let c = ctx.insert_vertex(at + v(0.0, 2.0, 0.0), edit);
            let d = ctx.insert_vertex(at + v(2.0, 0.0, 0.0), edit);
            ctx.insert_edge(a, b, edit).unwrap();
            ctx.insert_edge(c, d, edit).unwrap();
        })
        .unwrap();
}

/// A clean closed square (four edges, one face) drawn with the normal tools into `body`.
fn add_square(model: &mut Model, body: BodyId, at: DVec3) {
    let mut log = ModelEdit::new();
    let pts = [
        at + v(0.0, 0.0, 0.0),
        at + v(1.0, 0.0, 0.0),
        at + v(1.0, 1.0, 0.0),
        at + v(0.0, 1.0, 0.0),
    ];
    model
        .edit_geometry(body, &mut log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
}

fn place(model: &mut Model, body: BodyId, definition: DefinitionId, transform: DMat4) {
    let mut log = ModelEdit::new();
    model
        .add_instance(
            body,
            Instance {
                definition,
                transform,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
}

/// The two end points of `edge` in `body`, lower first (by x, then y, then z).
fn segment(model: &Model, body: BodyId, edge: EdgeId) -> [[i64; 3]; 2] {
    let geometry = model.body(body).unwrap().geometry();
    let e = geometry.edge(edge).unwrap();
    let mut ends = e.ends.map(|vertex| {
        let p = geometry.vertex(vertex).unwrap().position;
        [
            (p.x * 1000.0).round() as i64,
            (p.y * 1000.0).round() as i64,
            (p.z * 1000.0).round() as i64,
        ]
    });
    ends.sort();
    ends
}

/// Each `EdgesIntersect` in `body` as the pair of segments it names, sorted.
fn crossings(model: &Model, body: BodyId) -> Vec<[[[i64; 3]; 2]; 2]> {
    let mut out: Vec<_> = model
        .validate()
        .into_iter()
        .filter_map(|violation| match violation {
            ModelViolation::Geometry(b, Violation::EdgesIntersect(x, y)) if b == body => {
                let mut pair = [segment(model, b, x), segment(model, b, y)];
                pair.sort();
                Some(pair)
            }
            _ => None,
        })
        .collect();
    out.sort();
    out
}

/// The single definition that instances in `body` place (panics if there are several).
fn placed_definition(model: &Model, body: BodyId) -> DefinitionId {
    let mut placed: Vec<DefinitionId> = model
        .body(body)
        .unwrap()
        .instances()
        .map(|(_, instance)| instance.definition)
        .collect();
    placed.dedup();
    assert_eq!(placed.len(), 1, "{placed:?}");
    placed[0]
}

#[test]
fn crossing_edges_in_the_root_are_imported_as_they_are() {
    let mut source = Model::new();
    add_crossing(&mut source, BodyId::Root, DVec3::ZERO);
    assert_eq!(source.validate().len(), 1);
    let source_before = source.clone();

    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Key holder", &mut undo, "Import")
        .expect("a source's own crossing edges don't stop the import");
    let body = BodyId::Definition(id);
    assert_eq!(model.definition(id).unwrap().name, "Key holder");
    assert_eq!(model.body(body).unwrap().geometry().edge_count(), 2);
    assert_eq!(
        crossings(&model, body),
        crossings(&source, BodyId::Root),
        "the same two edges cross in the copy"
    );
    assert_eq!(model.validate().len(), 1, "{:?}", model.validate());
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(source, source_before, "source untouched");
}

#[test]
fn crossing_edges_next_to_clean_geometry_import() {
    let mut source = Model::new();
    add_square(&mut source, BodyId::Root, v(5.0, 0.0, 0.0));
    add_crossing(&mut source, BodyId::Root, DVec3::ZERO);
    add_crossing(&mut source, BodyId::Root, v(0.0, 5.0, 0.0));
    assert_eq!(source.validate().len(), 2);

    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Shed", &mut undo, "Import").unwrap();
    let body = BodyId::Definition(id);
    assert_eq!(model.body(body).unwrap().geometry().face_count(), 1);
    assert_eq!(crossings(&model, body), crossings(&source, BodyId::Root));
    assert_eq!(model.validate().len(), 2, "{:?}", model.validate());
}

#[test]
fn a_nested_definition_with_crossing_edges_comes_along() {
    let mut source = Model::new();
    let part = define(&mut source, "Part", false);
    add_crossing(&mut source, BodyId::Definition(part), DVec3::ZERO);
    add_square(&mut source, BodyId::Definition(part), v(5.0, 0.0, 0.0));
    place(&mut source, BodyId::Root, part, DMat4::IDENTITY);
    place(
        &mut source,
        BodyId::Root,
        part,
        DMat4::from_translation(v(10.0, 0.0, 0.0)),
    );
    assert_eq!(source.validate().len(), 1);

    let mut model = Model::new();
    define(&mut model, "Part", false);
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Assembly", &mut undo, "Import").unwrap();
    let copy = placed_definition(&model, BodyId::Definition(id));
    assert_eq!(model.definitions().count(), 3);
    assert_eq!(
        crossings(&model, BodyId::Definition(copy)),
        crossings(&source, BodyId::Definition(part))
    );
    assert_eq!(
        model.validate(),
        vec![
            model
                .validate()
                .into_iter()
                .find(|violation| matches!(
                    violation,
                    ModelViolation::Geometry(BodyId::Definition(d), Violation::EdgesIntersect(..))
                        if *d == copy
                ))
                .expect("the copy of Part carries the crossing")
        ],
        "nothing else is wrong"
    );
}

#[test]
fn every_problem_of_the_source_is_carried_and_nothing_more() {
    // Root: a crossing. Part (a component): a stray vertex, a crossing, and a pair of
    // coincident vertices that each end an edge (a duplicate vertex, and the two edges touch). Box (a group) placed twice. One instance with
    // a singular transform.
    let mut source = Model::new();
    add_crossing(&mut source, BodyId::Root, DVec3::ZERO);
    let part = define(&mut source, "Part", false);
    add_crossing(&mut source, BodyId::Definition(part), v(0.0, 0.0, 1.0));
    let mut log = ModelEdit::new();
    source
        .edit_geometry(BodyId::Definition(part), &mut log, |ctx, edit| {
            ctx.insert_vertex(v(9.0, 9.0, 9.0), edit);
            let a = ctx.insert_vertex(v(5.0, 0.0, 0.0), edit);
            let b = ctx.insert_vertex(v(6.0, 0.0, 0.0), edit);
            let c = ctx.insert_vertex(v(6.0, 0.0, 0.0), edit);
            let d = ctx.insert_vertex(v(6.0, 1.0, 0.0), edit);
            ctx.insert_edge(a, b, edit).unwrap();
            ctx.insert_edge(c, d, edit).unwrap();
        })
        .unwrap();
    let boxed = define(&mut source, "Box", true);
    add_square(&mut source, BodyId::Definition(boxed), DVec3::ZERO);
    place(&mut source, BodyId::Root, part, DMat4::IDENTITY);
    place(
        &mut source,
        BodyId::Root,
        part,
        DMat4::from_scale(v(1.0, 1.0, 0.0)),
    );
    place(&mut source, BodyId::Root, boxed, DMat4::IDENTITY);
    place(
        &mut source,
        BodyId::Root,
        boxed,
        DMat4::from_translation(v(0.0, 0.0, 3.0)),
    );
    let source_violations = source.validate();
    let kinds = |violations: &[ModelViolation]| {
        let mut kinds: Vec<String> = violations
            .iter()
            .map(|violation| match violation {
                ModelViolation::Geometry(_, g) => {
                    format!("{g:?}").split('(').next().unwrap().to_string()
                }
                ModelViolation::BadTransform(..) => "BadTransform".to_string(),
                ModelViolation::GroupInstances(_, n) => format!("GroupInstances {n}"),
            })
            .collect();
        kinds.sort();
        kinds
    };
    assert_eq!(
        kinds(&source_violations),
        vec![
            "BadTransform",
            "DuplicateVertex",
            "EdgesIntersect",
            "EdgesIntersect",
            "EdgesIntersect",
            "GroupInstances 2",
            "StrayVertex",
        ]
    );

    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Everything", &mut undo, "Import")
        .expect("only the source's own problems");
    let after = model.validate();
    assert_eq!(kinds(&after), kinds(&source_violations), "{after:?}");

    // Each carried problem names the copies of the source's entities.
    let root = BodyId::Definition(id);
    assert_eq!(crossings(&model, root), crossings(&source, BodyId::Root));
    let copies: Vec<DefinitionId> = {
        let mut d: Vec<DefinitionId> = model
            .body(root)
            .unwrap()
            .instances()
            .map(|(_, instance)| instance.definition)
            .collect();
        d.sort();
        d.dedup();
        d
    };
    let part_copy = *copies
        .iter()
        .find(|&&d| !model.definition(d).unwrap().group)
        .unwrap();
    let box_copy = *copies
        .iter()
        .find(|&&d| model.definition(d).unwrap().group)
        .unwrap();
    assert_eq!(
        crossings(&model, BodyId::Definition(part_copy)),
        crossings(&source, BodyId::Definition(part))
    );
    assert!(after.contains(&ModelViolation::GroupInstances(box_copy, 2)));
    let singular = model
        .body(root)
        .unwrap()
        .instances()
        .find(|(_, instance)| instance.transform.determinant().abs() < 1e-12)
        .unwrap()
        .0;
    assert!(after.contains(&ModelViolation::BadTransform(root, singular)));
    let part_geometry = model
        .body(BodyId::Definition(part_copy))
        .unwrap()
        .geometry();
    let stray = after
        .iter()
        .find_map(|violation| match violation {
            ModelViolation::Geometry(BodyId::Definition(d), Violation::StrayVertex(s))
                if *d == part_copy =>
            {
                Some(*s)
            }
            _ => None,
        })
        .expect("the stray vertex is in the copy of Part");
    assert_eq!(
        part_geometry.vertex(stray).unwrap().position,
        v(9.0, 9.0, 9.0)
    );
    let (a, b) = after
        .iter()
        .find_map(|violation| match violation {
            ModelViolation::Geometry(BodyId::Definition(d), Violation::DuplicateVertex(a, b))
                if *d == part_copy =>
            {
                Some((*a, *b))
            }
            _ => None,
        })
        .expect("the coincident pair is in the copy of Part");
    assert!(a < b, "lower id first");
    assert_eq!(part_geometry.vertex(a).unwrap().position, v(6.0, 0.0, 0.0));
    assert_eq!(part_geometry.vertex(b).unwrap().position, v(6.0, 0.0, 0.0));
}

#[test]
fn the_models_own_problems_are_left_as_they_were() {
    let mut model = Model::new();
    add_crossing(&mut model, BodyId::Root, v(0.0, 0.0, 7.0));
    let own = model.validate();
    assert_eq!(own.len(), 1);

    // A clean source adds nothing.
    let mut clean = Model::new();
    add_square(&mut clean, BodyId::Root, DVec3::ZERO);
    let mut undo = UndoStack::new();
    import_component(&mut model, &clean, "Slab", &mut undo, "Import").unwrap();
    assert_eq!(model.validate(), own);

    // A source with a crossing adds exactly that one, in the new definition.
    let mut crossed = Model::new();
    add_crossing(&mut crossed, BodyId::Root, DVec3::ZERO);
    let id = import_component(&mut model, &crossed, "Cross", &mut undo, "Import").unwrap();
    let after = model.validate();
    assert_eq!(after.len(), 2, "{after:?}");
    assert!(after.contains(&own[0]));
    assert_eq!(
        crossings(&model, BodyId::Definition(id)),
        crossings(&crossed, BodyId::Root)
    );
    assert_eq!(undo.undo_count(), 2);
}

#[test]
fn a_tolerant_import_is_one_undo_step_and_redo_restores_it() {
    let mut source = Model::new();
    let part = define(&mut source, "Part", false);
    add_crossing(&mut source, BodyId::Definition(part), DVec3::ZERO);
    place(&mut source, BodyId::Root, part, DMat4::IDENTITY);
    add_crossing(&mut source, BodyId::Root, v(4.0, 0.0, 0.0));

    let mut model = Model::new();
    add_square(&mut model, BodyId::Root, v(0.0, 0.0, 5.0));
    let before = model.clone();
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Part", &mut undo, "Import Component").unwrap();
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Import Component"));
    let after = model.clone();
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    assert!(model.definition(id).is_none());
    assert!(model.validate().is_empty());
    undo.redo(&mut model).unwrap();
    assert_eq!(model, after);
    assert_eq!(model.validate().len(), 2);
}

#[test]
fn importing_the_same_broken_source_twice_carries_its_problems_twice() {
    let mut source = Model::new();
    add_crossing(&mut source, BodyId::Root, DVec3::ZERO);
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let a = import_component(&mut model, &source, "Cross", &mut undo, "Import").unwrap();
    let b = import_component(&mut model, &source, "Cross", &mut undo, "Import").unwrap();
    assert_ne!(a, b);
    assert_eq!(crossings(&model, BodyId::Definition(a)).len(), 1);
    assert_eq!(crossings(&model, BodyId::Definition(b)).len(), 1);
    assert_eq!(model.validate().len(), 2);
}
